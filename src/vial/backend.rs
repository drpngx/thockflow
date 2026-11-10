//! Keyboard backends for the shared keyboard manager UI: plain Vial, and SVIL
//! (Svalboard's wrapped protocol). The UI talks to a [`Link`]; the VIA commands
//! both protocols share go through [`ViaTransport`].

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::rc::Rc;

use lzma_rs::{lzma_decompress, xz_decompress};
use serde_json::Value;
use vial_protocol::qmk_settings as qs;
use vial_protocol::vil::{self, VialKeyOverride, VilData};
use vial_protocol::{ComboEntry, DynamicEntryOp, TapDanceEntry, VialMessage};
use vial_protocol::svil::{
    self, file::SvilData, keycode_names::CustomKeycode, macros, menus, menus::CustomValueEntry, SvilCommand,
    SvilInfo, SvilTable,
};

use super::svil_link::SvilLink;
use super::webhid::{self, HidDevice};
use super::{MatrixPos, QmkSettingValue};

/// Which protocol a keyboard manager tab speaks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Protocol {
    Vial,
    Svil,
}

/// Keycode numbering used to display SVIL keymaps (QMK 0.19+, Vial protocol 6).
pub const SVIL_DISPLAY_PROTOCOL: u32 = 6;

// ---------------------------------------------------------------------------
// VIA transport shared by both protocols
// ---------------------------------------------------------------------------

/// Carries VIA commands. Replies start at the command echo.
#[allow(async_fn_in_trait)]
pub trait ViaTransport {
    async fn via(&self, payload: &[u8]) -> Result<Vec<u8>, String>;
    /// Data bytes one keymap get/set buffer message carries.
    fn buffer_chunk(&self) -> usize;
}

impl ViaTransport for HidDevice {
    async fn via(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        let mut msg = [0u8; vial_protocol::MSG_LEN];
        let n = payload.len().min(msg.len());
        msg[..n].copy_from_slice(&payload[..n]);
        webhid::send_message(self, &msg).await.map(|r| r.to_vec())
    }
    fn buffer_chunk(&self) -> usize {
        vial_protocol::BUFFER_FETCH_CHUNK
    }
}

impl ViaTransport for SvilLink {
    async fn via(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        self.send_via(payload).await
    }
    fn buffer_chunk(&self) -> usize {
        svil::payload_capacity(self.report_len(), svil::VIA_BUFFER_HEADER_BYTES)
    }
}

async fn via_protocol_version<T: ViaTransport>(t: &T) -> Result<u16, String> {
    let r = t.via(&[svil::via::GET_PROTOCOL_VERSION]).await?;
    Ok(u16::from_be_bytes([r.get(1).copied().unwrap_or(0), r.get(2).copied().unwrap_or(0)]))
}

async fn layer_count<T: ViaTransport>(t: &T) -> Result<u8, String> {
    let r = t.via(&[svil::via::GET_LAYER_COUNT]).await?;
    Ok(r.get(1).copied().unwrap_or(0))
}

/// Read the whole dynamic keymap with buffer reads (keycodes are big-endian).
async fn read_keymap<T: ViaTransport>(t: &T, layers: u8, rows: u8, cols: u8) -> Result<Vec<Vec<u16>>, String> {
    let per_layer = rows as usize * cols as usize;
    let total = layers as usize * per_layer * 2;
    let chunk = (t.buffer_chunk() & !1).clamp(2, 254);
    let mut bytes = Vec::with_capacity(total);
    let mut offset = 0usize;
    while offset < total {
        let size = chunk.min(total - offset);
        let r = t.via(&svil::keymap_get_buffer_payload(offset as u16, size as u8)).await?;
        let data = r.get(4..4 + size).ok_or("Short keymap buffer reply")?;
        bytes.extend_from_slice(data);
        offset += size;
    }
    let codes: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    Ok(codes.chunks(per_layer.max(1)).map(|c| c.to_vec()).collect())
}

/// Write keymaps with buffer writes, at most `device_layers` layers.
async fn write_keymap<T: ViaTransport>(t: &T, layers: &[Vec<u16>], per_layer: usize, device_layers: u8) -> Result<(), String> {
    let keys_per_msg = (t.buffer_chunk() / 2).max(1);
    let mut flat = Vec::new();
    for layer in layers.iter().take(device_layers as usize) {
        let mut l = layer.clone();
        l.resize(per_layer, 0);
        flat.extend(l);
    }
    for (i, block) in flat.chunks(keys_per_msg).enumerate() {
        let offset = (i * keys_per_msg * 2) as u16;
        t.via(&svil::keymap_set_buffer_payload(offset, block)).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// What a connected keyboard looks like to the UI
// ---------------------------------------------------------------------------

/// SVIL-only state.
#[derive(Clone, PartialEq, Default)]
pub struct SvilState {
    pub info: SvilInfo,
    pub via_protocol: u16,
    pub custom_keycodes: Vec<CustomKeycode>,
    /// VIA3 custom UI menus from the definition.
    pub menus: Option<Value>,
    /// Current values of every menu control, raw little-endian bytes.
    pub menu_values: Vec<CustomValueEntry>,
    /// Layer names as stored on the board.
    pub board_layer_names: Vec<String>,
    pub tap_dance_count: usize,
    pub combo_count: usize,
    pub key_override_count: usize,
    pub alt_repeat_key_count: usize,
    pub leader_count: usize,
    pub fragments: Option<Value>,
    pub composition: Option<Value>,
}

impl SvilState {
    pub fn keycode_names(&self) -> svil::keycode_names::KeycodeNames {
        svil::keycode_names::KeycodeNames::new(self.custom_keycodes.clone())
    }

    /// Menu values as integers by key, for `showIf`.
    pub fn menu_value_map(&self) -> HashMap<String, u32> {
        self.menu_values.iter().map(|e| (e.key.clone(), menus::bytes_to_int(&e.data))).collect()
    }
}

#[derive(Clone, PartialEq, Default)]
pub struct LoadedKeyboard {
    pub name: String,
    /// Keyboard UID (Vial `VIAL_KEYBOARD_UID`, or SVIL GET_INFO).
    pub uid: u64,
    pub vendor_id: u16,
    pub product_id: u16,
    pub microcontroller: String,
    /// Keycode numbering for display (the Vial protocol version).
    pub protocol_version: u32,
    pub rows: u8,
    pub cols: u8,
    pub layer_count: u8,
    pub layers: Vec<Vec<u16>>,
    pub key_layout: Vec<MatrixPos>,
    pub layer_names: Vec<String>,
    pub qmk_settings: Vec<QmkSettingValue>,
    pub definition_info: Option<String>,
    /// Non-fatal problems to show after connecting.
    pub notice: Option<String>,
    pub svil: Option<SvilState>,
}

/// Default display name of an unnamed layer.
pub fn default_layer_name(i: usize) -> String {
    format!("L{i}")
}

/// Parse `layouts.keymap` (KLE rows of `"row,col"` labels) into key positions.
pub fn parse_key_layout(json: &Value) -> Vec<MatrixPos> {
    let mut positions = Vec::new();
    let Some(rows) = json.get("layouts").and_then(|l| l.get("keymap")).and_then(|k| k.as_array()) else {
        return positions;
    };
    let mut cur_y = 0.0f32;
    for row_val in rows {
        if let Some(keys) = row_val.as_array() {
            let mut cur_x = 0.0f32;
            let mut cur_w = 1.0f32;
            let mut cur_h = 1.0f32;
            for k in keys {
                if let Some(obj) = k.as_object() {
                    let f = |key: &str| obj.get(key).and_then(|v| v.as_f64()).map(|v| v as f32);
                    if let Some(x) = f("x") {
                        cur_x += x;
                    }
                    if let Some(y) = f("y") {
                        cur_y += y;
                    }
                    if let Some(w) = f("w") {
                        cur_w = w;
                    }
                    if let Some(h) = f("h") {
                        cur_h = h;
                    }
                } else if let Some(label) = k.as_str() {
                    // Format: "row,col" or "label\n\nrow,col"
                    let coord = label.split('\n').next_back().unwrap_or("");
                    let coords: Vec<&str> = coord.split(',').collect();
                    if coords.len() == 2 {
                        let r: u8 = coords[0].trim().parse().unwrap_or(255);
                        let c: u8 = coords[1].trim().parse().unwrap_or(255);
                        if r != 255 && c != 255 {
                            positions.push(MatrixPos { row: r, col: c, x: cur_x, y: cur_y, w: cur_w, h: cur_h });
                        }
                    }
                    cur_x += cur_w;
                    cur_w = 1.0;
                    cur_h = 1.0;
                }
            }
        }
        cur_y += 1.0;
    }
    positions
}

fn apply_definition(json: &Value, kb: &mut LoadedKeyboard) {
    let s = |k: &str| json.get(k).and_then(|v| v.as_str()).map(str::to_string);
    if let Some(name) = s("name").or_else(|| s("keyboard_name")) {
        kb.name = name;
    }
    if let Some(mcu) = s("processor").or_else(|| s("mcu")) {
        kb.microcontroller = mcu;
    }
    if let Some(matrix) = json.get("matrix") {
        kb.rows = matrix.get("rows").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        kb.cols = matrix.get("cols").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    }
    kb.key_layout = parse_key_layout(json);
}

// ---------------------------------------------------------------------------
// Link
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Link {
    Vial(HidDevice),
    Svil(Rc<SvilLink>),
}

impl PartialEq for Link {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Link::Vial(a), Link::Vial(b)) => a == b,
            (Link::Svil(a), Link::Svil(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// Prompt for a keyboard and read everything the UI shows.
pub async fn connect(protocol: Protocol) -> Result<(Link, LoadedKeyboard), String> {
    match protocol {
        Protocol::Vial => connect_vial().await,
        Protocol::Svil => connect_svil().await,
    }
}

async fn connect_vial() -> Result<(Link, LoadedKeyboard), String> {
    let dev = webhid::request_device().await?;
    let mut kb = LoadedKeyboard {
        name: dev.product_name(),
        vendor_id: dev.vendor_id(),
        product_id: dev.product_id(),
        ..Default::default()
    };

    // 1. Keyboard ID: [vial_proto LE32][uid LE64]
    if let Ok(resp) = webhid::send_message(&dev, vial_protocol::VialMessage::get_keyboard_id().as_bytes()).await {
        let (ver, uid) = vial_protocol::parse_keyboard_id(&resp);
        kb.protocol_version = ver;
        kb.uid = uid;
        log::info!("Vial protocol v{ver}, UID {uid:016X}");
    }

    // 2. Definition size, then 32-byte definition blocks (XZ-compressed JSON)
    if let Ok(size_resp) = webhid::send_message(&dev, vial_protocol::VialMessage::get_size().as_bytes()).await {
        let total_size = vial_protocol::parse_definition_size(&size_resp) as usize;
        log::info!("Definition payload: {total_size} bytes");
        let mut payload = Vec::with_capacity(total_size);
        let mut block = 0u32;
        while payload.len() < total_size {
            match webhid::send_message(&dev, vial_protocol::VialMessage::get_definition(block).as_bytes()).await {
                Ok(resp) => {
                    let n = (total_size - payload.len()).min(vial_protocol::MSG_LEN);
                    payload.extend_from_slice(&resp[..n]);
                    block += 1;
                }
                Err(e) => {
                    log::warn!("definition fetch error at block {block}: {e}");
                    break;
                }
            }
        }
        let mut decompressed = Vec::new();
        match xz_decompress(&mut Cursor::new(&payload), &mut decompressed) {
            Ok(_) => {
                if let Ok(json) = serde_json::from_slice::<Value>(&decompressed) {
                    apply_definition(&json, &mut kb);
                    kb.definition_info = Some(format!(
                        "{} bytes LZMA payload, {} decompressed. Matrix: {}x{}",
                        payload.len(),
                        decompressed.len(),
                        kb.rows,
                        kb.cols
                    ));
                }
            }
            Err(e) => kb.notice = Some(format!("XZ decompression failed: {e}")),
        }
    }

    // 3. Layers and keymap
    kb.layer_count = layer_count(&dev).await.unwrap_or(0);
    if kb.layer_count > 0 && kb.rows > 0 && kb.cols > 0 {
        match read_keymap(&dev, kb.layer_count, kb.rows, kb.cols).await {
            Ok(layers) => kb.layers = layers,
            Err(e) => kb.notice = Some(format!("Couldn't read the keymap: {e}")),
        }
        kb.layer_names = (0..kb.layer_count as usize).map(default_layer_name).collect();
    }

    // 4. QMK settings (Vial protocol 4+)
    if kb.protocol_version >= vial_protocol::VIAL_PROTOCOL_QMK_SETTINGS {
        kb.qmk_settings = load_vial_qmk_settings(&dev).await;
    }
    Ok((Link::Vial(dev), kb))
}

async fn load_vial_qmk_settings(dev: &HidDevice) -> Vec<QmkSettingValue> {
    let mut supported: Vec<u16> = Vec::new();
    let mut cursor: u16 = 0;
    loop {
        let msg = vial_protocol::VialMessage::qmk_settings_query(cursor);
        let Ok(resp) = webhid::send_message(dev, msg.as_bytes()).await else { break };
        let qsids = vial_protocol::parse_qmk_settings_query(&resp);
        let max_qsid = qsids.iter().copied().fold(cursor, u16::max);
        for q in qsids {
            if !supported.contains(&q) {
                supported.push(q);
            }
        }
        if max_qsid == cursor {
            break;
        }
        cursor = max_qsid;
    }
    log::info!("QMK settings: {} supported qsids: {:?}", supported.len(), supported);
    let mut values = Vec::new();
    for &qsid in supported.iter().filter(|q| qs::is_qsid_supported(**q)) {
        let width = qs::width_for_qsid(qsid) as usize;
        match webhid::send_message(dev, vial_protocol::VialMessage::qmk_settings_get(qsid).as_bytes()).await {
            Ok(resp) => {
                let (_status, value) = vial_protocol::parse_qmk_settings_get(&resp, width);
                values.push(QmkSettingValue { qsid, value });
            }
            Err(e) => log::warn!("Failed to read qsid {qsid}: {e}"),
        }
    }
    values
}

async fn connect_svil() -> Result<(Link, LoadedKeyboard), String> {
    let link = Rc::new(SvilLink::request().await?);
    match load_svil(&link).await {
        Ok(kb) => Ok((Link::Svil(link), kb)),
        Err(e) => {
            link.close().await;
            Err(e)
        }
    }
}

async fn load_svil(link: &SvilLink) -> Result<LoadedKeyboard, String> {
    let dev = link.device();
    let mut kb = LoadedKeyboard {
        name: dev.product_name(),
        vendor_id: dev.vendor_id(),
        product_id: dev.product_id(),
        protocol_version: SVIL_DISPLAY_PROTOCOL,
        ..Default::default()
    };
    let mut st = SvilState { via_protocol: via_protocol_version(link).await?, ..Default::default() };

    // 1. GET_INFO
    let info_reply = link.svil(&[SvilCommand::GetInfo as u8]).await?;
    st.info = SvilInfo::parse(&info_reply).ok_or("The keyboard sent an invalid GET_INFO reply.")?;
    st.info.check_supported()?;
    kb.uid = st.info.uid;
    log::info!("SVIL protocol v{}, UID {:016X}, keycodes {}", st.info.protocol_version, st.info.uid, st.info.keycode_version_string());
    let mut notices = Vec::new();
    if st.info.keycode_version_string() != svil::KEYCODE_VERSION {
        notices.push(format!(
            "This keyboard numbers keycodes as QMK {}; keys are shown as QMK {}.",
            st.info.keycode_version_string(),
            svil::KEYCODE_VERSION
        ));
    }
    if st.info.storage_reset() || st.info.storage_build_reset() {
        notices.push("The keyboard reset its settings to defaults (unreadable storage or a new firmware build).".into());
    }
    if st.info.storage_write_failed() {
        notices.push("The keyboard couldn't save a change to flash since it started; changes will be lost on restart.".into());
    }

    // 2. Definition: raw LZMA, read in chunks as large as the report allows
    let size = svil::parse_u32_after_echo(&link.svil(&[SvilCommand::DefinitionSize as u8]).await?) as usize;
    if !(svil::LZMA_HEADER_SIZE..=50 * 1024 * 1024).contains(&size) {
        return Err(format!("Invalid keyboard definition size: {size}"));
    }
    let chunk = svil::payload_capacity(link.report_len(), svil::SVIL_DEFINITION_HEADER_BYTES).min(255);
    let mut payload = Vec::with_capacity(size);
    while payload.len() < size {
        let want = chunk.min(size - payload.len());
        let reply = link.svil(&svil::definition_chunk_payload(payload.len() as u16, want as u8)).await?;
        let data = svil::parse_definition_chunk(&reply, want);
        if data.is_empty() {
            break;
        }
        payload.extend_from_slice(data);
    }
    svil::check_lzma_header(&payload)?;
    let mut decompressed = Vec::new();
    lzma_decompress(&mut Cursor::new(&payload), &mut decompressed)
        .map_err(|e| format!("Couldn't decompress the keyboard definition: {e}"))?;
    let json: Value =
        serde_json::from_slice(&decompressed).map_err(|e| format!("Invalid keyboard definition: {e}"))?;
    apply_definition(&json, &mut kb);
    kb.definition_info = Some(format!(
        "SVIL v{} · {} bytes LZMA definition, {} decompressed · Matrix: {}x{} · {}-byte reports",
        st.info.protocol_version,
        payload.len(),
        decompressed.len(),
        kb.rows,
        kb.cols,
        link.report_len()
    ));
    st.custom_keycodes = CustomKeycode::from_definition(&json);
    if let Some(counts) = json.get("sval").or_else(|| json.get("viable")) {
        let n = |k: &str| counts.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        st.tap_dance_count = n("tap_dance");
        st.combo_count = n("combo");
        st.key_override_count = n("key_override");
        st.alt_repeat_key_count = n("alt_repeat_key");
        st.leader_count = n("leader");
    }
    st.menus = json.get("menus").filter(|m| m.is_array()).cloned();
    st.fragments = json.get("fragments").cloned();
    st.composition = json.get("composition").cloned();
    register_custom_keycodes(&st.custom_keycodes);

    // 3. Layers and keymap
    kb.layer_count = layer_count(link).await?;
    if kb.layer_count == 0 || kb.rows == 0 || kb.cols == 0 {
        return Err("The keyboard reported no layers or an empty matrix.".into());
    }
    kb.layers = read_keymap(link, kb.layer_count, kb.rows, kb.cols).await?;

    // 4. Layer names stored on the board
    let mut board_names = vec![String::new(); kb.layer_count as usize];
    for (i, name) in scan_labels(link, svil::LABEL_TYPE_LAYER).await.unwrap_or_default() {
        if let Some(slot) = board_names.get_mut(i as usize) {
            *slot = name;
        }
    }
    kb.layer_names = board_names
        .iter()
        .enumerate()
        .map(|(i, n)| if n.is_empty() { default_layer_name(i) } else { n.clone() })
        .collect();
    st.board_layer_names = board_names;

    // 5. QMK settings
    kb.qmk_settings = load_svil_qmk_settings(link).await;

    // 6. Keyboard settings (VIA3 menus)
    if let Some(m) = &st.menus {
        st.menu_values = read_menu_values(link, m).await;
    }

    kb.notice = (!notices.is_empty()).then(|| notices.join(" "));
    kb.svil = Some(st);
    Ok(kb)
}

/// Make the keyboard's own keycodes show and parse by name in the keymap UI.
pub fn register_custom_keycodes(custom: &[CustomKeycode]) {
    let entries = custom
        .iter()
        .enumerate()
        .take(svil::keycode_names::QK_USER_COUNT)
        .filter(|(_, c)| !c.name.is_empty())
        .map(|(i, c)| (svil::keycode_names::QK_USER + i as u16, c.label().to_string(), c.name.clone()))
        .collect();
    vial_protocol::keycodes::set_custom_keycodes(entries);
}

async fn scan_labels(link: &SvilLink, label_type: u8) -> Result<Vec<(u16, String)>, String> {
    let mut out = Vec::new();
    let mut start: u32 = 0;
    while start <= 0xFFFF {
        let reply = link.svil(&svil::label_scan_payload(label_type, start as u16)).await?;
        let Some((index, name)) = svil::parse_label_scan(&reply) else { break };
        if (index as u32) < start {
            break;
        }
        out.push((index, name));
        start = index as u32 + 1;
    }
    Ok(out)
}

async fn load_svil_qmk_settings(link: &SvilLink) -> Vec<QmkSettingValue> {
    let mut supported: Vec<u16> = Vec::new();
    let mut cursor: u16 = 0;
    loop {
        let Ok(reply) = link.svil(&svil::qmk_settings_query_payload(cursor)).await else { break };
        let qsids = svil::parse_qmk_settings_query(&reply);
        let max_qsid = qsids.iter().copied().fold(cursor, u16::max);
        for q in qsids {
            if !supported.contains(&q) {
                supported.push(q);
            }
        }
        if max_qsid == cursor {
            break;
        }
        cursor = max_qsid;
    }
    let mut values = Vec::new();
    for &qsid in supported.iter().filter(|q| qs::is_qsid_supported(**q)) {
        let width = qs::width_for_qsid(qsid) as usize;
        match link.svil(&svil::qmk_settings_get_payload(qsid)).await {
            Ok(reply) => {
                let (status, value) = svil::parse_qmk_settings_get(&reply, width);
                if status == 0 {
                    values.push(QmkSettingValue { qsid, value });
                }
            }
            Err(e) => log::warn!("Failed to read qsid {qsid}: {e}"),
        }
    }
    values
}

async fn read_menu_values(link: &SvilLink, m: &Value) -> Vec<CustomValueEntry> {
    let mut out: Vec<CustomValueEntry> = Vec::new();
    for (item, r) in menus::controls(m) {
        if out.iter().any(|e| e.key == r.key) {
            continue;
        }
        let width = menus::byte_width(item);
        match link.send(svil::VIA_PREFIX, &svil::custom_get_payload(r.channel, r.value_id), 3).await {
            Ok(reply) => {
                let mut data: Vec<u8> = reply.iter().skip(3).take(width).copied().collect();
                data.resize(width, 0);
                out.push(CustomValueEntry { key: r.key, channel: r.channel, value_id: r.value_id, data });
            }
            Err(e) => log::warn!("Failed to read custom value {}: {e}", r.key),
        }
    }
    out
}

async fn read_table(link: &SvilLink, table: &SvilTable, count: usize) -> Result<Vec<Vec<u8>>, String> {
    let mut entries = vec![vec![0u8; table.entry_size]; count];
    let mut start = 0usize;
    while start < count {
        let reply = link.svil(&svil::table_scan_payload(table, start as u16)).await?;
        let Some((index, entry)) = svil::parse_table_scan(&reply, table) else { break };
        let index = index as usize;
        if index < start || index >= count {
            break;
        }
        entries[index][..entry.len()].copy_from_slice(entry);
        start = index + 1;
    }
    Ok(entries)
}

async fn read_macros(link: &SvilLink) -> Result<Vec<Vec<macros::MacroAction>>, String> {
    let count_reply = link.send_via(&[svil::via::MACRO_GET_COUNT]).await?;
    let count = count_reply.get(1).copied().unwrap_or(0) as usize | (count_reply.get(2).copied().unwrap_or(0) as usize) << 8;
    if count == 0 {
        return Ok(Vec::new());
    }
    let size = svil::parse_u32_after_echo(&link.svil(&[SvilCommand::MacroBufferSize as u8]).await?) as usize;
    let chunk = svil::payload_capacity(link.report_len(), svil::SVIL_MACRO_BUFFER_HEADER_BYTES).min(255);
    let mut buffer = Vec::new();
    let mut terminators = 0;
    while buffer.len() < size && terminators < count {
        let offset = buffer.len() as u32;
        let n = chunk.min(size - buffer.len()) as u8;
        let reply = link.svil(&svil::macro_buffer_get_payload(offset, n)).await?;
        let data = svil::parse_macro_buffer_get(&reply, offset, n)?;
        if data.is_empty() {
            break;
        }
        terminators += data.iter().filter(|&&b| b == 0).count();
        buffer.extend_from_slice(data);
    }
    let mut parsed: Vec<_> = macros::split(&buffer, count).into_iter().map(macros::parse).collect();
    parsed.resize(count, Vec::new());
    Ok(parsed)
}

impl Link {
    pub async fn close(&self) {
        match self {
            Link::Vial(d) => {
                let _ = webhid::close_device(d).await;
            }
            Link::Svil(l) => l.close().await,
        }
    }

    pub async fn poll_matrix(&self, rows: u8, cols: u8) -> Result<Vec<Vec<bool>>, String> {
        match self {
            Link::Vial(d) => {
                let resp = webhid::send_message(d, vial_protocol::VialMessage::get_switch_matrix_state().as_bytes()).await?;
                Ok(vial_protocol::parse_matrix_state(&resp, rows, cols))
            }
            Link::Svil(l) => {
                let reply = l.send_via(&[svil::via::GET_KEYBOARD_VALUE, svil::via::SWITCH_MATRIX_STATE]).await?;
                Ok(svil::parse_matrix_state(&reply, rows, cols))
            }
        }
    }

    /// Write the editor's keymap (at most `device_layers` layers).
    pub async fn write_keymap(&self, layers: &[Vec<u16>], rows: u8, cols: u8, device_layers: u8) -> Result<(), String> {
        let per_layer = rows as usize * cols as usize;
        match self {
            Link::Vial(d) => write_keymap(d, layers, per_layer, device_layers).await,
            Link::Svil(l) => write_keymap(l.as_ref(), layers, per_layer, device_layers).await,
        }
    }

    /// Store changed layer names on the board (SVIL); returns the names it now holds.
    pub async fn write_layer_names(&self, names: &[String], board: &[String]) -> Result<Vec<String>, String> {
        let Link::Svil(l) = self else { return Ok(board.to_vec()) };
        let mut now = board.to_vec();
        for (i, stored) in now.iter_mut().enumerate() {
            let wanted = names.get(i).cloned().unwrap_or_default();
            let wanted = if wanted == default_layer_name(i) { String::new() } else { wanted };
            if wanted == *stored {
                continue;
            }
            svil::validate_label(&wanted).map_err(|e| format!("Layer {i} name: {e}"))?;
            let cmd = if wanted.is_empty() { SvilCommand::LabelClear } else { SvilCommand::LabelSet };
            let reply = l.svil(&svil::label_set_payload(svil::LABEL_TYPE_LAYER, i as u16, &wanted)).await?;
            svil::check_status(cmd, &reply)?;
            *stored = wanted;
        }
        Ok(now)
    }

    pub async fn set_qmk_setting(&self, qsid: u16, value: &[u8]) -> Result<(), String> {
        match self {
            Link::Vial(d) => {
                let msg = vial_protocol::VialMessage::qmk_settings_set(qsid, value);
                webhid::send_message(d, msg.as_bytes()).await.map(|_| ())
            }
            Link::Svil(l) => l.svil(&svil::qmk_settings_set_payload(qsid, value)).await.map(|_| ()),
        }
    }

    pub async fn reset_qmk_settings(&self) -> Result<(), String> {
        match self {
            Link::Vial(d) => {
                webhid::send_message(d, vial_protocol::VialMessage::qmk_settings_reset().as_bytes()).await.map(|_| ())
            }
            Link::Svil(l) => l.svil(&[SvilCommand::QmkSettingsReset as u8]).await.map(|_| ()),
        }
    }

    /// Set a VIA custom value and save its channel (SVIL keyboard settings).
    pub async fn set_custom_value(&self, channel: u8, value_id: u8, data: &[u8]) -> Result<(), String> {
        let Link::Svil(l) = self else { return Err("Custom values need an SVIL keyboard".into()) };
        l.send(svil::VIA_PREFIX, &svil::custom_set_payload(channel, value_id, data), 1).await?;
        l.send(svil::VIA_PREFIX, &svil::custom_save_payload(channel), 1).await?;
        Ok(())
    }

    /// Everything for a `.svil` file: the editor's keymap, names and settings,
    /// plus the tables (macros, tap dances, combos, ...) read from the board.
    pub async fn read_svil_backup(&self, kb: &LoadedKeyboard) -> Result<SvilData, String> {
        let (Link::Svil(l), Some(st)) = (self, &kb.svil) else {
            return Err("Saving .svil needs an SVIL keyboard".into());
        };
        let mut data = SvilData {
            uid: st.info.uid,
            svil_protocol: st.info.protocol_version,
            via_protocol: st.via_protocol,
            keycode_version: svil::KEYCODE_VERSION.to_string(),
            layout: svil::file::layout_from_flat(&kb.layers, kb.rows as usize, kb.cols as usize),
            menus: st.menus.clone(),
            fragments: st.fragments.clone(),
            composition: st.composition.clone(),
            custom_values: st.menu_values.clone(),
            ..Default::default()
        };
        data.layer_names = kb
            .layer_names
            .iter()
            .enumerate()
            .filter(|(i, n)| !n.is_empty() && **n != default_layer_name(*i))
            .map(|(i, n)| (i, n.clone()))
            .collect();
        data.settings = kb
            .qmk_settings
            .iter()
            .map(|s| (s.qsid, menus::bytes_to_int(&s.value)))
            .collect::<BTreeMap<_, _>>();
        data.macros = read_macros(l).await?;
        data.tap_dances = read_table(l, &svil::TABLE_TAP_DANCE, st.tap_dance_count)
            .await?
            .iter()
            .map(|e| svil::TapDance::from_bytes(e))
            .collect();
        data.combos =
            read_table(l, &svil::TABLE_COMBO, st.combo_count).await?.iter().map(|e| svil::Combo::from_bytes(e)).collect();
        data.key_overrides = read_table(l, &svil::TABLE_KEY_OVERRIDE, st.key_override_count)
            .await?
            .iter()
            .map(|e| svil::KeyOverride::from_bytes(e))
            .collect();
        if st.alt_repeat_key_count > 0 {
            data.alt_repeat_keys = read_table(l, &svil::TABLE_ALT_REPEAT_KEY, st.alt_repeat_key_count)
                .await?
                .iter()
                .map(|e| svil::AltRepeatKey::from_bytes(e))
                .collect();
        }
        if st.info.has_leader() && st.leader_count > 0 {
            data.leaders = read_table(l, &svil::TABLE_LEADER, st.leader_count)
                .await?
                .iter()
                .map(|e| svil::Leader::from_bytes(e))
                .collect();
        }
        if st.info.has_oneshot() {
            let reply = l.svil(&[SvilCommand::OneShotGet as u8]).await?;
            let (timeout, tap_toggle) = svil::parse_one_shot(&reply);
            data.one_shot = Some(svil::OneShot { timeout, tap_toggle });
        }
        Ok(data)
    }
}

// ---------------------------------------------------------------------------
// Migration: full .vil backups from Vial boards, table writes to SVIL boards
// ---------------------------------------------------------------------------

async fn vial_dynamic(dev: &HidDevice, op: DynamicEntryOp, index: u8) -> Result<Vec<u8>, String> {
    let resp = webhid::send_message(dev, VialMessage::dynamic_entry_op(op, index).as_bytes()).await?;
    Ok(resp.to_vec())
}

/// Read the VIA macro buffer (Vial firmware): `count` NUL-terminated macros.
async fn read_via_macros(dev: &HidDevice) -> Result<Vec<Vec<macros::MacroAction>>, String> {
    let count = dev.via(&[svil::via::MACRO_GET_COUNT]).await?.get(1).copied().unwrap_or(0) as usize;
    if count == 0 {
        return Ok(Vec::new());
    }
    let r = dev.via(&[svil::via::MACRO_GET_BUFFER_SIZE]).await?;
    let size = u16::from_be_bytes([r.get(1).copied().unwrap_or(0), r.get(2).copied().unwrap_or(0)]) as usize;
    let chunk = dev.buffer_chunk();
    let mut buffer = Vec::new();
    let mut terminators = 0;
    while buffer.len() < size && terminators < count {
        let n = chunk.min(size - buffer.len());
        let o = (buffer.len() as u16).to_be_bytes();
        let r = dev.via(&[0x0E, o[0], o[1], n as u8]).await?;
        let data = r.get(4..4 + n).ok_or("Short macro buffer reply")?;
        terminators += data.iter().filter(|&&b| b == 0).count();
        buffer.extend_from_slice(data);
    }
    let mut parsed: Vec<_> = macros::split(&buffer, count).into_iter().map(macros::parse).collect();
    parsed.resize(count, Vec::new());
    Ok(parsed)
}

/// What [`Link::write_svil_tables`] did.
#[derive(Debug, Default)]
pub struct TableWriteReport {
    pub written: usize,
    /// Entries the board has no room for.
    pub dropped: usize,
    pub macros_written: bool,
}

impl Link {
    /// A full Vial `.vil` backup: the editor's keymap and layer names, plus macros,
    /// tap dances, combos, key overrides and QMK settings read from the board.
    pub async fn read_vil_backup(&self, kb: &LoadedKeyboard) -> Result<VilData, String> {
        let Link::Vial(dev) = self else { return Err("Saving .vil needs a Vial keyboard".into()) };
        let (rows, cols) = (kb.rows as usize, kb.cols as usize);
        let present = |r: usize, c: usize| {
            kb.key_layout.is_empty() || kb.key_layout.iter().any(|p| p.row as usize == r && p.col as usize == c)
        };
        let layout = kb
            .layers
            .iter()
            .map(|l| {
                (0..rows)
                    .map(|r| {
                        (0..cols).map(|c| present(r, c).then(|| l.get(r * cols + c).copied().unwrap_or(0))).collect()
                    })
                    .collect()
            })
            .collect();
        let mut data = VilData {
            uid: kb.uid,
            vial_protocol: kb.protocol_version,
            via_protocol: via_protocol_version(dev).await.unwrap_or(9),
            layout,
            encoder_layout: vec![Vec::new(); kb.layers.len()],
            layout_options: -1,
            ..Default::default()
        };
        if let Ok(r) = dev.via(&[svil::via::GET_KEYBOARD_VALUE, vial_protocol::KeyboardValue::LayoutOptions as u8]).await {
            let v = u32::from_be_bytes([r[2], r[3], r[4], r[5]]);
            if v != 0 {
                data.layout_options = v as i64;
            }
        }
        data.layer_names = kb
            .layer_names
            .iter()
            .enumerate()
            .filter(|(i, n)| !n.is_empty() && **n != default_layer_name(*i))
            .map(|(i, n)| (i, n.clone()))
            .collect();
        data.settings = kb.qmk_settings.iter().map(|s| (s.qsid, menus::bytes_to_int(&s.value))).collect();
        data.macros = read_via_macros(dev).await?;

        // Dynamic entries (Vial protocol 4+)
        if kb.protocol_version >= 4 {
            let (td, combo, ko) = vil::parse_dynamic_counts(&vial_dynamic(dev, DynamicEntryOp::GetNumberOfEntries, 0).await?);
            for i in 0..td {
                let r = vial_dynamic(dev, DynamicEntryOp::TapDanceGet, i).await?;
                let e = vil::parse_dynamic_entry(&r).and_then(TapDanceEntry::from_bytes).ok_or("Couldn't read a tap dance")?;
                data.tap_dances.push(e);
            }
            for i in 0..combo {
                let r = vial_dynamic(dev, DynamicEntryOp::ComboGet, i).await?;
                let e = vil::parse_dynamic_entry(&r).and_then(ComboEntry::from_bytes).ok_or("Couldn't read a combo")?;
                data.combos.push(e);
            }
            for i in 0..ko {
                let r = vial_dynamic(dev, DynamicEntryOp::KeyOverrideGet, i).await?;
                let e = vil::parse_dynamic_entry(&r).and_then(VialKeyOverride::from_bytes).ok_or("Couldn't read a key override")?;
                data.key_overrides.push(e);
            }
        }
        Ok(data)
    }

    /// Write the macros and tables of a layout file (tap dances, combos, key
    /// overrides, alt-repeat keys, leaders, one-shot) to an SVIL board. Only
    /// entries that differ from the board are written; the board then saves.
    pub async fn write_svil_tables(&self, data: &SvilData, st: &SvilState) -> Result<TableWriteReport, String> {
        let Link::Svil(l) = self else { return Err("Needs an SVIL keyboard".into()) };
        let mut report = TableWriteReport::default();

        async fn write_table<T: PartialEq>(
            l: &SvilLink,
            table: &SvilTable,
            count: usize,
            wanted: &[T],
            parse: impl Fn(&[u8]) -> T,
            payload: impl Fn(u16, &T) -> Vec<u8>,
            set_cmd: SvilCommand,
            report: &mut TableWriteReport,
        ) -> Result<(), String> {
            if wanted.is_empty() {
                return Ok(());
            }
            report.dropped += wanted.len().saturating_sub(count);
            let current = read_table(l, table, count).await?;
            for (i, entry) in wanted.iter().enumerate().take(count) {
                if parse(&current[i]) == *entry {
                    continue;
                }
                let reply = l.svil(&payload(i as u16, entry)).await?;
                svil::check_status(set_cmd, &reply)?;
                report.written += 1;
            }
            Ok(())
        }

        write_table(l, &svil::TABLE_TAP_DANCE, st.tap_dance_count, &data.tap_dances, svil::TapDance::from_bytes,
            svil::tap_dance_set_payload, SvilCommand::TapDanceSet, &mut report).await?;
        write_table(l, &svil::TABLE_COMBO, st.combo_count, &data.combos, svil::Combo::from_bytes,
            svil::combo_set_payload, SvilCommand::ComboSet, &mut report).await?;
        write_table(l, &svil::TABLE_KEY_OVERRIDE, st.key_override_count, &data.key_overrides, svil::KeyOverride::from_bytes,
            svil::key_override_set_payload, SvilCommand::KeyOverrideSet, &mut report).await?;
        write_table(l, &svil::TABLE_ALT_REPEAT_KEY, st.alt_repeat_key_count, &data.alt_repeat_keys, svil::AltRepeatKey::from_bytes,
            svil::alt_repeat_key_set_payload, SvilCommand::AltRepeatKeySet, &mut report).await?;
        let leader_count = if st.info.has_leader() { st.leader_count } else { 0 };
        write_table(l, &svil::TABLE_LEADER, leader_count, &data.leaders, svil::Leader::from_bytes,
            svil::leader_set_payload, SvilCommand::LeaderSet, &mut report).await?;

        if let (Some(os), true) = (data.one_shot, st.info.has_oneshot()) {
            let current = svil::OneShot::parse(&l.svil(&[SvilCommand::OneShotGet as u8]).await?);
            if current != os {
                l.svil(&svil::one_shot_set_payload(&os)).await?;
                report.written += 1;
            }
        }

        // Macros: the whole buffer, if it differs.
        let current_macros = read_macros(l).await?;
        let count = current_macros.len();
        let mut wanted: Vec<_> = data.macros.iter().take(count).cloned().collect();
        report.dropped += data.macros.iter().skip(count).filter(|m| !m.is_empty()).count();
        wanted.resize(count, Vec::new());
        if count > 0 && wanted != current_macros {
            let buffer = macros::dump(&wanted);
            let size = svil::parse_u32_after_echo(&l.svil(&[SvilCommand::MacroBufferSize as u8]).await?) as usize;
            if buffer.len() > size {
                return Err(format!("The macros need {} bytes; the keyboard has room for {size}.", buffer.len()));
            }
            for payload in svil::macro_buffer_set_payloads(&buffer, count, l.report_len()) {
                let reply = l.svil(&payload).await?;
                svil::check_status(SvilCommand::MacroBufferSet, &reply)?;
            }
            report.macros_written = true;
        }

        l.svil(&[SvilCommand::Save as u8]).await?;
        Ok(report)
    }
}
