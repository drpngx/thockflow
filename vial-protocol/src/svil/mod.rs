//! SVIL (Svalboard Vial) protocol.
//!
//! SVIL keeps the VIA command set but carries every report inside a client-ID
//! wrapper so several apps can share the keyboard, and replaces Vial's `0xFE`
//! sub-commands with its own `0xDF` command set.
//!
//! Reference implementation: Keybard
//! (<https://github.com/svalboard/keybard/blob/main/src/services/usb.service.ts>).
//!
//! Report layout:
//! ```text
//! [0xDD][client_id:4 LE][protocol][payload...]      protocol = 0xFE (VIA) | 0xDF (SVIL)
//! [0xDD][client_id:4 LE][0xFF][error_code]          error frame
//! [0xDD][0 0 0 0][nonce:20]                         client-ID bootstrap request
//! [0xDD][0 0 0 0][nonce:20][client_id:4][ttl:2]     bootstrap reply
//! ```
//!
//! This module only builds and parses payloads; the transport lives in the app.

pub mod diagnostics;
pub mod fragments;
pub mod keycode_names;
pub mod macros;
#[cfg(feature = "std")]
pub mod file;
#[cfg(feature = "std")]
pub mod menus;
mod qmk_names;

pub use qmk_names::KEYCODE_VERSION;

// ---------------------------------------------------------------------------
// Framing constants
// ---------------------------------------------------------------------------

pub const WRAPPER_PREFIX: u8 = 0xDD;
pub const SVIL_PREFIX: u8 = 0xDF;
pub const VIA_PREFIX: u8 = 0xFE;
/// Protocol byte of a wrapper error frame.
pub const CLIENT_ERROR_PROTOCOL: u8 = 0xFF;
/// QMK's `id_unhandled` reply prefix.
pub const QMK_UNHANDLED: u8 = 0xFF;

/// `[0xDD][client_id:4][protocol]`.
pub const WRAPPER_HEADER_BYTES: usize = 6;
pub const NONCE_SIZE: usize = 20;

/// Report length used when the HID descriptor cannot be read (today's firmware).
pub const DEFAULT_REPORT_LENGTH: usize = 32;
/// The smallest report the protocol fits in (the bootstrap uses bytes 0-30).
pub const MIN_REPORT_LENGTH: usize = 32;

/// VIA get/set buffer framing: `[wrapper:6][cmd][offset:2][size][data...]`.
pub const VIA_BUFFER_HEADER_BYTES: usize = WRAPPER_HEADER_BYTES + 4;
/// SVIL DEFINITION_CHUNK reply: `[wrapper:6][cmd][offset:2][actual_size][data...]`.
pub const SVIL_DEFINITION_HEADER_BYTES: usize = WRAPPER_HEADER_BYTES + 4;
/// SVIL MACRO_BUFFER_GET reply / SET request: `[wrapper:6][cmd][offset:4][count][data...]`.
pub const SVIL_MACRO_BUFFER_HEADER_BYTES: usize = WRAPPER_HEADER_BYTES + 6;

/// Oldest SVIL protocol version supported (1 and 2 were never released).
pub const MIN_SVIL_PROTO: u32 = 3;

/// The firmware ages client IDs in ~65 s steps, so renew well before that.
pub const CLIENT_ID_RENEW_SECS: u32 = 50;

/// Wrapper error codes.
pub const CLIENT_ERR_INVALID_ID: u8 = 0x01;
pub const CLIENT_ERR_NO_IDS: u8 = 0x02;
pub const CLIENT_ERR_UNKNOWN_PROTO: u8 = 0x03;

/// Raw HID interfaces SVIL firmware exposes, as `(usage_page, usage)`, in
/// order of preference.
pub const RAW_HID_USAGES: &[(u16, u16)] = &[(0xFF61, 0x62), (0xFF60, 0x61), (0xFF60, 0x62)];

/// Bytes left for data in one report after `header_bytes` of framing.
pub fn payload_capacity(report_len: usize, header_bytes: usize) -> usize {
    report_len.max(MIN_REPORT_LENGTH).saturating_sub(header_bytes)
}

// ---------------------------------------------------------------------------
// Commands and feature flags
// ---------------------------------------------------------------------------

/// SVIL command IDs (sent after the `0xDF` protocol byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SvilCommand {
    GetInfo = 0x00,
    TapDanceGet = 0x01,
    TapDanceSet = 0x02,
    ComboGet = 0x03,
    ComboSet = 0x04,
    KeyOverrideGet = 0x05,
    KeyOverrideSet = 0x06,
    AltRepeatKeyGet = 0x07,
    AltRepeatKeySet = 0x08,
    OneShotGet = 0x09,
    OneShotSet = 0x0A,
    Save = 0x0B,
    Reset = 0x0C,
    DefinitionSize = 0x0D,
    DefinitionChunk = 0x0E,
    QmkSettingsQuery = 0x10,
    QmkSettingsGet = 0x11,
    QmkSettingsSet = 0x12,
    QmkSettingsReset = 0x13,
    LeaderGet = 0x14,
    LeaderSet = 0x15,
    LayerStateGet = 0x16,
    LayerStateSet = 0x17,
    FragmentGetHardware = 0x18,
    FragmentGetSelections = 0x19,
    FragmentSetSelections = 0x1A,
    LabelGet = 0x1B,
    LabelSet = 0x1C,
    LabelClear = 0x1D,
    MacroBufferSize = 0x1E,
    MacroBufferGet = 0x1F,
    MacroBufferSet = 0x20,
    TableScan = 0x21,
    StorageResetClear = 0x2A,
    DefaultLayerSet = 0x2B,
    PointingHardware = 0x2C,
    PointingMotion = 0x2D,
    PointingContacts = 0x2F,
    PointingSensor = 0x39,
    TypingPause = 0x3C,
    DebugConsole = 0x3E,
}

/// GET_INFO `feature_flags`.
pub const FLAG_CAPS_WORD: u8 = 0x01;
pub const FLAG_LAYER_LOCK: u8 = 0x02;
pub const FLAG_ONESHOT: u8 = 0x04;
pub const FLAG_LEADER: u8 = 0x08;
pub const FLAG_CONTEXT_LAYER: u8 = 0x20;
pub const FLAG_DEFAULT_LAYER_STATE: u8 = 0x40;
/// GET_INFO `feature_flags2`.
pub const FLAG2_DEFAULT_LAYER_SET: u8 = 0x01;
pub const FLAG2_COMBO_LABELS: u8 = 0x80;

/// Label types for LABEL_* (each label is a 16-byte, NUL-padded UTF-8 field).
pub const LABEL_TYPE_LAYER: u8 = 0;
pub const LABEL_TYPE_TAP_DANCE: u8 = 1;
pub const LABEL_TYPE_MACRO: u8 = 2;
pub const LABEL_TYPE_COMBO: u8 = 3;
pub const LABEL_SIZE: usize = 16;

/// A SVIL table: its TABLE_SCAN id, per-index GET command and entry size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvilTable {
    pub scan_id: u8,
    pub get_cmd: SvilCommand,
    pub entry_size: usize,
}

pub const TABLE_TAP_DANCE: SvilTable =
    SvilTable { scan_id: 0, get_cmd: SvilCommand::TapDanceGet, entry_size: 10 };
pub const TABLE_COMBO: SvilTable =
    SvilTable { scan_id: 1, get_cmd: SvilCommand::ComboGet, entry_size: 12 };
pub const TABLE_KEY_OVERRIDE: SvilTable =
    SvilTable { scan_id: 2, get_cmd: SvilCommand::KeyOverrideGet, entry_size: 12 };
pub const TABLE_ALT_REPEAT_KEY: SvilTable =
    SvilTable { scan_id: 3, get_cmd: SvilCommand::AltRepeatKeyGet, entry_size: 6 };
pub const TABLE_LEADER: SvilTable =
    SvilTable { scan_id: 4, get_cmd: SvilCommand::LeaderGet, entry_size: 14 };

// ---------------------------------------------------------------------------
// Wrapper framing
// ---------------------------------------------------------------------------

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b.get(at).copied().unwrap_or(0), b.get(at + 1).copied().unwrap_or(0)])
}

fn le32(b: &[u8], at: usize) -> u32 {
    let g = |i: usize| b.get(at + i).copied().unwrap_or(0);
    u32::from_le_bytes([g(0), g(1), g(2), g(3)])
}

/// Build `[0xDD][client_id][protocol][payload...]`, zero-padded to `report_len`.
pub fn build_wrapped(report_len: usize, client_id: u32, protocol: u8, payload: &[u8]) -> Vec<u8> {
    let len = report_len.max(MIN_REPORT_LENGTH);
    let mut msg = vec![0u8; len];
    msg[0] = WRAPPER_PREFIX;
    msg[1..5].copy_from_slice(&client_id.to_le_bytes());
    msg[5] = protocol;
    let n = payload.len().min(len - WRAPPER_HEADER_BYTES);
    msg[WRAPPER_HEADER_BYTES..WRAPPER_HEADER_BYTES + n].copy_from_slice(&payload[..n]);
    msg
}

/// Build the client-ID bootstrap request `[0xDD][0 0 0 0][nonce:20]`.
pub fn build_bootstrap(report_len: usize, nonce: &[u8; NONCE_SIZE]) -> Vec<u8> {
    let mut msg = vec![0u8; report_len.max(MIN_REPORT_LENGTH)];
    msg[0] = WRAPPER_PREFIX;
    msg[5..5 + NONCE_SIZE].copy_from_slice(nonce);
    msg
}

/// What a report means for a pending bootstrap request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapReply {
    /// Not an answer to our request (another client, stray traffic).
    Other,
    /// QMK answered `id_unhandled`: this firmware has no wrapper.
    Unhandled,
    /// Our request came back unchanged (old Svalboard Vial firmware echoes).
    Echoed,
    /// The firmware handed out ID 0, which it won't accept; ask again.
    ZeroId,
    /// The firmware refused with an error code.
    Error(u8),
    Granted { client_id: u32, ttl_secs: u16 },
}

/// Classify a report received after sending `request` (built by [`build_bootstrap`]).
pub fn parse_bootstrap_reply(resp: &[u8], request: &[u8]) -> BootstrapReply {
    if resp.len() < 31 || request.len() < 5 + NONCE_SIZE {
        return BootstrapReply::Other;
    }
    if resp[0] == QMK_UNHANDLED && resp[1..5 + NONCE_SIZE] == request[1..5 + NONCE_SIZE] {
        return BootstrapReply::Unhandled;
    }
    if resp[0] != WRAPPER_PREFIX || le32(resp, 1) != 0 {
        return BootstrapReply::Other;
    }
    if resp[5..5 + NONCE_SIZE] != request[5..5 + NONCE_SIZE] {
        return BootstrapReply::Other;
    }
    let client_id = le32(resp, 25);
    let ttl_secs = le16(resp, 29);
    match client_id {
        0 if ttl_secs == 0 => BootstrapReply::Echoed,
        0 => BootstrapReply::ZeroId,
        0xFFFF_FFFF => BootstrapReply::Error(resp[29]),
        _ => BootstrapReply::Granted { client_id, ttl_secs },
    }
}

/// A report as seen by a client waiting for a wrapped reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrappedReply<'a> {
    /// Not wrapped, or addressed to another client.
    NotOurs,
    /// Error frame addressed to us.
    ClientError(u8),
    /// Our reply; `data` starts after the protocol byte (at the command echo).
    Payload { protocol: u8, data: &'a [u8] },
}

pub fn classify_reply(resp: &[u8], client_id: u32) -> WrappedReply<'_> {
    if resp.len() < WRAPPER_HEADER_BYTES + 1 || resp[0] != WRAPPER_PREFIX || le32(resp, 1) != client_id {
        return WrappedReply::NotOurs;
    }
    if resp[5] == CLIENT_ERROR_PROTOCOL {
        return WrappedReply::ClientError(resp[6]);
    }
    WrappedReply::Payload { protocol: resp[5], data: &resp[WRAPPER_HEADER_BYTES..] }
}

/// Renew a lease at 90% of its TTL, but never later than [`CLIENT_ID_RENEW_SECS`].
pub fn lease_millis(ttl_secs: u16) -> f64 {
    (ttl_secs as f64 * 900.0).min(CLIENT_ID_RENEW_SECS as f64 * 1000.0)
}

// ---------------------------------------------------------------------------
// GET_INFO
// ---------------------------------------------------------------------------

/// The reply to GET_INFO:
/// `[echo][proto:4][uid:8][flags][keycodes major,minor,patch][storage][flags2][flags3][flags4]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SvilInfo {
    pub protocol_version: u32,
    pub uid: u64,
    pub feature_flags: u8,
    /// QMK keycode numbering the board uses (None on firmware that doesn't say).
    pub keycode_version: Option<(u8, u8, u8)>,
    pub storage_flags: u8,
    pub feature_flags2: u8,
    pub feature_flags3: u8,
    pub feature_flags4: u8,
}

impl SvilInfo {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 14 || payload[0] != SvilCommand::GetInfo as u8 {
            return None;
        }
        let g = |i: usize| payload.get(i).copied().unwrap_or(0);
        let mut uid_bytes = [0u8; 8];
        uid_bytes.copy_from_slice(&payload[5..13]);
        let kv = (g(14), g(15), g(16));
        Some(Self {
            protocol_version: le32(payload, 1),
            uid: u64::from_le_bytes(uid_bytes),
            feature_flags: g(13),
            keycode_version: if kv == (0, 0, 0) { None } else { Some(kv) },
            storage_flags: g(17),
            feature_flags2: g(18),
            feature_flags3: g(19),
            feature_flags4: g(20),
        })
    }

    /// Keycode version as `"0.0.9"`; firmware that doesn't report one used 0.0.9.
    pub fn keycode_version_string(&self) -> String {
        match self.keycode_version {
            Some((a, b, c)) => format!("{a}.{b}.{c}"),
            None => KEYCODE_VERSION.to_string(),
        }
    }

    /// The board reset its settings because its storage could not be read.
    pub fn storage_reset(&self) -> bool {
        self.storage_flags & 0x01 != 0
    }
    /// Since it started, the board couldn't save a change to flash.
    pub fn storage_write_failed(&self) -> bool {
        self.storage_flags & 0x02 != 0
    }
    /// The first boot of a different firmware build reset every setting.
    pub fn storage_build_reset(&self) -> bool {
        self.storage_flags & 0x04 != 0
    }
    pub fn has_oneshot(&self) -> bool {
        self.feature_flags & FLAG_ONESHOT != 0
    }
    pub fn has_leader(&self) -> bool {
        self.feature_flags & FLAG_LEADER != 0
    }
    /// Problems that make this board unusable, if any.
    pub fn check_supported(&self) -> Result<(), String> {
        if self.protocol_version == 0 {
            return Err("The keyboard didn't answer like Svalboard QMK firmware.".into());
        }
        if self.protocol_version < MIN_SVIL_PROTO {
            return Err(format!(
                "This keyboard runs a pre-release SVIL build (protocol {}); update its firmware.",
                self.protocol_version
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Payload builders (the part after the protocol byte)
// ---------------------------------------------------------------------------

/// `[cmd][args...]`.
pub fn svil_payload(cmd: SvilCommand, args: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(1 + args.len());
    v.push(cmd as u8);
    v.extend_from_slice(args);
    v
}

/// Table / label index bytes (protocol 2+ uses 2-byte little-endian indices).
pub fn index_args(index: u16) -> [u8; 2] {
    index.to_le_bytes()
}

/// `[0x0E][offset:2 LE][size]`.
pub fn definition_chunk_payload(offset: u16, size: u8) -> Vec<u8> {
    let o = offset.to_le_bytes();
    svil_payload(SvilCommand::DefinitionChunk, &[o[0], o[1], size])
}

/// Data of a DEFINITION_CHUNK reply `[echo][offset:2][actual_size][data...]`.
/// A chunk may be shorter than requested; an empty one means the end.
pub fn parse_definition_chunk(payload: &[u8], requested: usize) -> &[u8] {
    if payload.len() < 4 {
        return &[];
    }
    let actual = (payload[3] as usize).min(requested).min(payload.len() - 4);
    &payload[4..4 + actual]
}

/// A `u32` right after the command echo (DEFINITION_SIZE, MACRO_BUFFER_SIZE).
pub fn parse_u32_after_echo(payload: &[u8]) -> u32 {
    le32(payload, 1)
}

/// Raw LZMA ("lzma-alone") header: properties, dictionary size, uncompressed size.
pub const LZMA_HEADER_SIZE: usize = 13;
const LZMA_MAX_PROPERTIES: u8 = 9 * 5 * 5 - 1;

/// Sanity check of a compressed definition before handing it to the decoder.
pub fn check_lzma_header(data: &[u8]) -> Result<(), String> {
    if data.len() < LZMA_HEADER_SIZE || data[0] > LZMA_MAX_PROPERTIES {
        return Err(format!(
            "The keyboard sent an unreadable keyboard definition ({} bytes).",
            data.len()
        ));
    }
    Ok(())
}

/// `[0x10][cursor:2 LE]`.
pub fn qmk_settings_query_payload(cursor: u16) -> Vec<u8> {
    svil_payload(SvilCommand::QmkSettingsQuery, &cursor.to_le_bytes())
}

/// QSIDs in a QMK_SETTINGS_QUERY reply `[echo][qsid:2 LE...]`, up to the 0xFFFF terminator.
pub fn parse_qmk_settings_query(payload: &[u8]) -> Vec<u16> {
    payload
        .get(1..)
        .unwrap_or(&[])
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&q| q != 0xFFFF)
        .collect()
}

/// `[0x11][qsid:2 LE]`.
pub fn qmk_settings_get_payload(qsid: u16) -> Vec<u8> {
    svil_payload(SvilCommand::QmkSettingsGet, &qsid.to_le_bytes())
}

/// `(status, value bytes)` of a QMK_SETTINGS_GET reply `[echo][status][value...]`.
pub fn parse_qmk_settings_get(payload: &[u8], width: usize) -> (u8, Vec<u8>) {
    let status = payload.get(1).copied().unwrap_or(0xFF);
    let mut value: Vec<u8> = payload.get(2..).unwrap_or(&[]).iter().copied().take(width).collect();
    value.resize(width, 0);
    (status, value)
}

/// `[0x12][qsid:2 LE][value:4 LE]` (the value is always sent as 4 bytes).
pub fn qmk_settings_set_payload(qsid: u16, value: &[u8]) -> Vec<u8> {
    let mut args = qsid.to_le_bytes().to_vec();
    let mut v = [0u8; 4];
    for (i, b) in value.iter().take(4).enumerate() {
        v[i] = *b;
    }
    args.extend_from_slice(&v);
    svil_payload(SvilCommand::QmkSettingsSet, &args)
}

/// `[0x1B][type][start:2 LE]` (protocol 2+: returns the next non-empty label).
pub fn label_scan_payload(label_type: u8, start: u16) -> Vec<u8> {
    let s = start.to_le_bytes();
    svil_payload(SvilCommand::LabelGet, &[label_type, s[0], s[1]])
}

/// `Some((index, label))` from a LABEL_GET scan reply
/// `[echo][type][found][index:2][label:16]`, `None` when nothing is left.
pub fn parse_label_scan(payload: &[u8]) -> Option<(u16, String)> {
    if payload.len() < 5 || payload[2] == 0 {
        return None;
    }
    let index = le16(payload, 3);
    let end = (5 + LABEL_SIZE).min(payload.len());
    Some((index, decode_label(&payload[5..end])))
}

/// `[0x1C][type][index:2][label:16]`, or LABEL_CLEAR `[0x1D][type][index:2]` for an empty name.
pub fn label_set_payload(label_type: u8, index: u16, label: &str) -> Vec<u8> {
    let i = index.to_le_bytes();
    if label.is_empty() {
        return svil_payload(SvilCommand::LabelClear, &[label_type, i[0], i[1]]);
    }
    let mut args = vec![label_type, i[0], i[1]];
    args.extend_from_slice(&encode_label(label));
    svil_payload(SvilCommand::LabelSet, &args)
}

/// UTF-8 encode into the fixed label field without splitting a character.
pub fn encode_label(label: &str) -> [u8; LABEL_SIZE] {
    let mut out = [0u8; LABEL_SIZE];
    let mut n = 0;
    for ch in label.chars() {
        let mut buf = [0u8; 4];
        let bytes = ch.encode_utf8(&mut buf).as_bytes();
        if n + bytes.len() > LABEL_SIZE {
            break;
        }
        out[n..n + bytes.len()].copy_from_slice(bytes);
        n += bytes.len();
    }
    out
}

pub fn decode_label(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Check a name the board can store: up to 16 UTF-8 bytes, no control characters.
pub fn validate_label(name: &str) -> Result<(), String> {
    if name.len() > LABEL_SIZE {
        return Err("Names can use up to 16 UTF-8 bytes.".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("Names cannot contain control characters.".into());
    }
    Ok(())
}

/// `[0x21][table][start:2 LE]`.
pub fn table_scan_payload(table: &SvilTable, start: u16) -> Vec<u8> {
    let s = start.to_le_bytes();
    svil_payload(SvilCommand::TableScan, &[table.scan_id, s[0], s[1]])
}

/// `Some((index, entry))` from a TABLE_SCAN reply
/// `[echo][table][found][index:2][entry...]`, `None` when nothing is left.
pub fn parse_table_scan<'a>(payload: &'a [u8], table: &SvilTable) -> Option<(u16, &'a [u8])> {
    if payload.len() < 5 || payload[2] == 0 {
        return None;
    }
    let end = (5 + table.entry_size).min(payload.len());
    Some((le16(payload, 3), &payload[5..end]))
}

/// `[0x1F][offset:4 LE][count]`.
pub fn macro_buffer_get_payload(offset: u32, count: u8) -> Vec<u8> {
    let mut args = offset.to_le_bytes().to_vec();
    args.push(count);
    svil_payload(SvilCommand::MacroBufferGet, &args)
}

/// Data of a MACRO_BUFFER_GET reply `[echo][offset:4][count][data...]`, or an
/// error when the board refused (it answers a status in place of the offset).
pub fn parse_macro_buffer_get(payload: &[u8], offset: u32, count: u8) -> Result<&[u8], String> {
    if payload.len() < 6 || le32(payload, 1) != offset || payload[5] != count {
        return Err(format!(
            "Macro buffer read at {offset} refused (status {})",
            payload.get(1).copied().unwrap_or(0xFF)
        ));
    }
    let end = (6 + count as usize).min(payload.len());
    Ok(&payload[6..end])
}

/// `(timeout_ms, tap_toggle)` of a ONE_SHOT_GET reply `[echo][timeout:2][tap_toggle]`.
pub fn parse_one_shot(payload: &[u8]) -> (u16, u8) {
    (le16(payload, 1), payload.get(3).copied().unwrap_or(0))
}

/// Status byte of a SVIL write reply `[echo][status]`: `Ok` for 0.
pub fn check_status(cmd: SvilCommand, payload: &[u8]) -> Result<(), String> {
    match payload.get(1) {
        Some(0) => Ok(()),
        Some(s) => Err(format!("Keyboard refused SVIL command 0x{:02x} (status {s})", cmd as u8)),
        None => Err(format!("Empty reply to SVIL command 0x{:02x}", cmd as u8)),
    }
}

// ---------------------------------------------------------------------------
// VIA commands carried in the wrapper (protocol byte 0xFE)
// ---------------------------------------------------------------------------

/// VIA command bytes used over SVIL (same values as [`crate::ViaCommand`]).
pub mod via {
    pub const GET_PROTOCOL_VERSION: u8 = 0x01;
    pub const GET_KEYBOARD_VALUE: u8 = 0x02;
    pub const SET_KEYBOARD_VALUE: u8 = 0x03;
    pub const SET_KEYCODE: u8 = 0x05;
    pub const CUSTOM_SET_VALUE: u8 = 0x07;
    pub const CUSTOM_GET_VALUE: u8 = 0x08;
    pub const CUSTOM_SAVE: u8 = 0x09;
    pub const MACRO_GET_COUNT: u8 = 0x0C;
    pub const MACRO_GET_BUFFER_SIZE: u8 = 0x0D;
    pub const GET_LAYER_COUNT: u8 = 0x11;
    pub const KEYMAP_GET_BUFFER: u8 = 0x12;
    pub const KEYMAP_SET_BUFFER: u8 = 0x13;
    pub const SWITCH_MATRIX_STATE: u8 = 0x03;
}

/// `[0x12][offset:2 BE][size]`.
pub fn keymap_get_buffer_payload(offset: u16, size: u8) -> Vec<u8> {
    let o = offset.to_be_bytes();
    vec![via::KEYMAP_GET_BUFFER, o[0], o[1], size]
}

/// `[0x13][offset:2 BE][size][keycode:2 BE...]`.
pub fn keymap_set_buffer_payload(offset: u16, keycodes: &[u16]) -> Vec<u8> {
    let o = offset.to_be_bytes();
    let mut v = vec![via::KEYMAP_SET_BUFFER, o[0], o[1], (keycodes.len() * 2) as u8];
    for kc in keycodes {
        v.extend_from_slice(&kc.to_be_bytes());
    }
    v
}

/// `[0x08][channel][value_id]`.
pub fn custom_get_payload(channel: u8, value_id: u8) -> Vec<u8> {
    vec![via::CUSTOM_GET_VALUE, channel, value_id]
}

/// `[0x07][channel][value_id][data...]`.
pub fn custom_set_payload(channel: u8, value_id: u8, data: &[u8]) -> Vec<u8> {
    let mut v = vec![via::CUSTOM_SET_VALUE, channel, value_id];
    v.extend_from_slice(data);
    v
}

/// `[0x09][channel]`.
pub fn custom_save_payload(channel: u8) -> Vec<u8> {
    vec![via::CUSTOM_SAVE, channel]
}

/// Parse a wrapped switch-matrix reply `[0x02][0x03][offset][rows...]`.
///
/// Each row is `ceil(cols/8)` bytes, most significant byte first.
pub fn parse_matrix_state(payload: &[u8], rows: u8, cols: u8) -> Vec<Vec<bool>> {
    let row_bytes = (cols as usize).div_ceil(8);
    let mut offset = 0;
    if payload.len() >= 2 && payload[0] == via::GET_KEYBOARD_VALUE && payload[1] == via::SWITCH_MATRIX_STATE {
        offset = 3;
    }
    let mut matrix = vec![vec![false; cols as usize]; rows as usize];
    for (r, row) in matrix.iter_mut().enumerate() {
        let start = offset + r * row_bytes;
        if start + row_bytes > payload.len() {
            break;
        }
        let data = &payload[start..start + row_bytes];
        for (c, cell) in row.iter_mut().enumerate() {
            let byte = row_bytes - 1 - c / 8;
            *cell = data[byte] & (1 << (c % 8)) != 0;
        }
    }
    matrix
}

// ---------------------------------------------------------------------------
// Table entries
// ---------------------------------------------------------------------------

/// Tap dance entry `[tap:2][hold:2][double_tap:2][tap_hold:2][term:2]` (term bit 15 = enabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TapDance {
    pub on_tap: u16,
    pub on_hold: u16,
    pub on_double_tap: u16,
    pub on_tap_hold: u16,
    pub tapping_term: u16,
    pub enabled: bool,
}

impl TapDance {
    pub fn from_bytes(d: &[u8]) -> Self {
        let term = le16(d, 8);
        Self {
            on_tap: le16(d, 0),
            on_hold: le16(d, 2),
            on_double_tap: le16(d, 4),
            on_tap_hold: le16(d, 6),
            tapping_term: term & 0x7FFF,
            enabled: term & 0x8000 != 0,
        }
    }
}

/// Combo entry `[key0..3:2][output:2][options:2]` (options bit 15 = enabled, 0-14 = term).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Combo {
    pub keys: [u16; 4],
    pub output: u16,
    pub options: u16,
}

pub const COMBO_ENABLED: u16 = 0x8000;

impl Combo {
    pub fn from_bytes(d: &[u8]) -> Self {
        Self {
            keys: [le16(d, 0), le16(d, 2), le16(d, 4), le16(d, 6)],
            output: le16(d, 8),
            options: le16(d, 10),
        }
    }
}

/// Key override entry
/// `[trigger:2][replacement:2][layers:4][trigger_mods][negative_mod_mask][suppressed_mods][options]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyOverride {
    pub trigger: u16,
    pub replacement: u16,
    pub layers: u32,
    pub trigger_mods: u8,
    pub negative_mod_mask: u8,
    pub suppressed_mods: u8,
    pub options: u8,
}

/// Key override options bit 7: enabled.
pub const KEY_OVERRIDE_ENABLED: u8 = 0x80;

impl KeyOverride {
    pub fn from_bytes(d: &[u8]) -> Self {
        let g = |i: usize| d.get(i).copied().unwrap_or(0);
        Self {
            trigger: le16(d, 0),
            replacement: le16(d, 2),
            layers: le32(d, 4),
            trigger_mods: g(8),
            negative_mod_mask: g(9),
            suppressed_mods: g(10),
            options: g(11),
        }
    }
}

/// Alt repeat key entry `[keycode:2][alt_keycode:2][allowed_mods][options]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AltRepeatKey {
    pub keycode: u16,
    pub alt_keycode: u16,
    pub allowed_mods: u8,
    pub options: u8,
}

impl AltRepeatKey {
    pub fn from_bytes(d: &[u8]) -> Self {
        Self {
            keycode: le16(d, 0),
            alt_keycode: le16(d, 2),
            allowed_mods: d.get(4).copied().unwrap_or(0),
            options: d.get(5).copied().unwrap_or(0),
        }
    }
}

/// Leader entry `[seq0..4:2][output:2][options:2]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Leader {
    pub sequence: [u16; 5],
    pub output: u16,
    pub options: u16,
}

impl Leader {
    pub fn from_bytes(d: &[u8]) -> Self {
        Self {
            sequence: [le16(d, 0), le16(d, 2), le16(d, 4), le16(d, 6), le16(d, 8)],
            output: le16(d, 10),
            options: le16(d, 12),
        }
    }
}

/// One-shot settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OneShot {
    pub timeout: u16,
    pub tap_toggle: u8,
}

// ---------------------------------------------------------------------------
// Entry serialization and the write side of every table
// ---------------------------------------------------------------------------

impl TapDance {
    /// Wire form; the term carries the enabled flag in bit 15 (terms clamp to 15 bits).
    pub fn to_bytes(&self) -> [u8; 10] {
        let term = (self.tapping_term & 0x7FFF) | if self.enabled { 0x8000 } else { 0 };
        let mut b = [0u8; 10];
        for (i, v) in [self.on_tap, self.on_hold, self.on_double_tap, self.on_tap_hold, term].iter().enumerate() {
            b[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

impl Combo {
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut b = [0u8; 12];
        for (i, v) in self.keys.iter().chain([self.output, self.options].iter()).enumerate() {
            b[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    pub fn enabled(&self) -> bool {
        self.options & COMBO_ENABLED != 0
    }
    /// Per-combo term in ms (0 = the global combo term).
    pub fn term(&self) -> u16 {
        self.options & 0x7FFF
    }
}

impl KeyOverride {
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut b = [0u8; 12];
        b[0..2].copy_from_slice(&self.trigger.to_le_bytes());
        b[2..4].copy_from_slice(&self.replacement.to_le_bytes());
        b[4..8].copy_from_slice(&self.layers.to_le_bytes());
        b[8] = self.trigger_mods;
        b[9] = self.negative_mod_mask;
        b[10] = self.suppressed_mods;
        b[11] = self.options;
        b
    }
    pub fn enabled(&self) -> bool {
        self.options & KEY_OVERRIDE_ENABLED != 0
    }
}

impl AltRepeatKey {
    pub fn to_bytes(&self) -> [u8; 6] {
        let k = self.keycode.to_le_bytes();
        let a = self.alt_keycode.to_le_bytes();
        [k[0], k[1], a[0], a[1], self.allowed_mods, self.options]
    }
}

impl Leader {
    pub fn to_bytes(&self) -> [u8; 14] {
        let mut b = [0u8; 14];
        for (i, v) in self.sequence.iter().chain([self.output, self.options].iter()).enumerate() {
            b[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

impl OneShot {
    pub fn parse(payload: &[u8]) -> Self {
        let (timeout, tap_toggle) = parse_one_shot(payload);
        Self { timeout, tap_toggle }
    }
}

/// Per-index GET of a table: `[get_cmd][index:2]` (firmware without TABLE_SCAN).
pub fn table_get_payload(table: &SvilTable, index: u16) -> Vec<u8> {
    svil_payload(table.get_cmd, &index_args(index))
}

/// Entry of a per-index GET reply `[echo][index:2][entry...]`.
pub fn parse_table_get<'a>(payload: &'a [u8], table: &SvilTable) -> &'a [u8] {
    let end = (3 + table.entry_size).min(payload.len());
    payload.get(3..end).unwrap_or(&[])
}

/// `[set_cmd][index:2][entry...]`; the reply is `[echo][status]` (see [`check_status`]).
pub fn table_set_payload(set_cmd: SvilCommand, index: u16, entry: &[u8]) -> Vec<u8> {
    let mut args = index_args(index).to_vec();
    args.extend_from_slice(entry);
    svil_payload(set_cmd, &args)
}

pub fn tap_dance_set_payload(index: u16, td: &TapDance) -> Vec<u8> {
    table_set_payload(SvilCommand::TapDanceSet, index, &td.to_bytes())
}
pub fn combo_set_payload(index: u16, c: &Combo) -> Vec<u8> {
    table_set_payload(SvilCommand::ComboSet, index, &c.to_bytes())
}
pub fn key_override_set_payload(index: u16, k: &KeyOverride) -> Vec<u8> {
    table_set_payload(SvilCommand::KeyOverrideSet, index, &k.to_bytes())
}
pub fn alt_repeat_key_set_payload(index: u16, a: &AltRepeatKey) -> Vec<u8> {
    table_set_payload(SvilCommand::AltRepeatKeySet, index, &a.to_bytes())
}
pub fn leader_set_payload(index: u16, l: &Leader) -> Vec<u8> {
    table_set_payload(SvilCommand::LeaderSet, index, &l.to_bytes())
}

/// `[0x0A][timeout:2][tap_toggle]`.
pub fn one_shot_set_payload(os: &OneShot) -> Vec<u8> {
    let t = os.timeout.to_le_bytes();
    svil_payload(SvilCommand::OneShotSet, &[t[0], t[1], os.tap_toggle])
}

/// `[0x20][offset:4][count][data...]`; reply `[echo][status]` (0 ok, 1 out of range).
pub fn macro_buffer_set_payload(offset: u32, data: &[u8]) -> Vec<u8> {
    let mut args = offset.to_le_bytes().to_vec();
    args.push(data.len() as u8);
    args.extend_from_slice(data);
    svil_payload(SvilCommand::MacroBufferSet, &args)
}

/// Split a macro buffer write into MACRO_BUFFER_SET payloads that fit the report.
///
/// Only the used part of the buffer (through the `count`-th terminator) is written.
pub fn macro_buffer_set_payloads(buffer: &[u8], count: usize, report_len: usize) -> Vec<Vec<u8>> {
    let mut used = 0;
    let mut terminators = 0;
    while used < buffer.len() && terminators < count.max(1) {
        if buffer[used] == 0 {
            terminators += 1;
        }
        used += 1;
    }
    let chunk = payload_capacity(report_len, SVIL_MACRO_BUFFER_HEADER_BYTES).clamp(1, 255);
    buffer[..used]
        .chunks(chunk)
        .enumerate()
        .map(|(i, c)| macro_buffer_set_payload((i * chunk) as u32, c))
        .collect()
}

/// Layer state from LAYER_STATE_GET `[echo][active:4][default:4]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayerState {
    pub active: u32,
    /// Only on firmware with [`FLAG_DEFAULT_LAYER_STATE`].
    pub default: Option<u32>,
}

impl LayerState {
    pub fn parse(payload: &[u8], has_default: bool) -> Self {
        Self { active: le32(payload, 1), default: has_default.then(|| le32(payload, 5)) }
    }
    /// Highest layer turned on (counting the default layer state), 0 when none.
    pub fn top_layer(&self) -> u8 {
        highest_layer(self.active | self.default.unwrap_or(0))
    }
    pub fn default_layer(&self) -> Option<u8> {
        self.default.map(highest_layer)
    }
}

/// Highest set bit of a layer mask (0 for an empty mask).
pub fn highest_layer(mask: u32) -> u8 {
    if mask == 0 { 0 } else { 31 - mask.leading_zeros() as u8 }
}

/// `[0x17][mask:4]`: set the live layer state (not saved).
pub fn layer_state_set_payload(mask: u32) -> Vec<u8> {
    svil_payload(SvilCommand::LayerStateSet, &mask.to_le_bytes())
}

/// `[0x2B][layer]`: set the default layer (needs [`FLAG2_DEFAULT_LAYER_SET`]); reply `[echo][status]`.
pub fn default_layer_set_payload(layer: u8) -> Vec<u8> {
    svil_payload(SvilCommand::DefaultLayerSet, &[layer])
}

/// SAVE (`0x0B`), RESET (`0x0C`), STORAGE_RESET_CLEAR (`0x2A`) and other argument-less commands.
pub fn simple_payload(cmd: SvilCommand) -> Vec<u8> {
    svil_payload(cmd, &[])
}

impl SvilInfo {
    pub fn has_default_layer_state(&self) -> bool {
        self.feature_flags & FLAG_DEFAULT_LAYER_STATE != 0
    }
    pub fn has_default_layer_set(&self) -> bool {
        self.feature_flags2 & FLAG2_DEFAULT_LAYER_SET != 0
    }
    pub fn has_combo_labels(&self) -> bool {
        self.feature_flags2 & FLAG2_COMBO_LABELS != 0
    }
    pub fn has_caps_word(&self) -> bool {
        self.feature_flags & FLAG_CAPS_WORD != 0
    }
    pub fn has_layer_lock(&self) -> bool {
        self.feature_flags & FLAG_LAYER_LOCK != 0
    }
    pub fn has_context_layer(&self) -> bool {
        self.feature_flags & FLAG_CONTEXT_LAYER != 0
    }
}

/// `[0x05][layer][row][col][keycode:2 BE]`.
pub fn set_keycode_payload(layer: u8, row: u8, col: u8, keycode: u16) -> Vec<u8> {
    let k = keycode.to_be_bytes();
    vec![via::SET_KEYCODE, layer, row, col, k[0], k[1]]
}

/// Macro count from VIA MACRO_GET_COUNT `[echo][count lo][count hi]` (256-macro firmware sends the high byte).
pub fn parse_macro_count(payload: &[u8]) -> u16 {
    le16(payload, 1)
}

/// Data of a VIA custom value reply `[0x08][channel][value_id][data...]`, if it answers that value.
pub fn parse_custom_get(payload: &[u8], channel: u8, value_id: u8, width: usize) -> Option<Vec<u8>> {
    if payload.len() < 3 || payload[0] != via::CUSTOM_GET_VALUE || payload[1] != channel || payload[2] != value_id {
        return None;
    }
    let mut data: Vec<u8> = payload[3..].iter().take(width).copied().collect();
    data.resize(width, 0);
    Some(data)
}

/// VIA keymap buffer: data of a GET_BUFFER reply `[0x12][offset:2][size][data...]`.
pub fn parse_keymap_get_buffer(payload: &[u8], size: usize) -> &[u8] {
    let end = (4 + size).min(payload.len());
    payload.get(4..end).unwrap_or(&[])
}

/// Keycodes per VIA keymap SET_BUFFER message (11 at 32-byte reports, 27 at 64).
pub fn keymap_block_keys(report_len: usize) -> usize {
    payload_capacity(report_len, VIA_BUFFER_HEADER_BYTES) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_roundtrip() {
        let msg = build_wrapped(32, 0x1234_5678, VIA_PREFIX, &[0x11]);
        assert_eq!(msg.len(), 32);
        assert_eq!(&msg[..7], &[0xDD, 0x78, 0x56, 0x34, 0x12, 0xFE, 0x11]);
        let mut reply = msg.clone();
        reply[7] = 16;
        match classify_reply(&reply, 0x1234_5678) {
            WrappedReply::Payload { protocol, data } => {
                assert_eq!(protocol, VIA_PREFIX);
                assert_eq!(&data[..2], &[0x11, 16]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(classify_reply(&reply, 1), WrappedReply::NotOurs);
        reply[5] = 0xFF;
        reply[6] = CLIENT_ERR_INVALID_ID;
        assert_eq!(classify_reply(&reply, 0x1234_5678), WrappedReply::ClientError(1));
    }

    #[test]
    fn wrapped_64_byte_reports() {
        let payload: Vec<u8> = (0..70).collect();
        let msg = build_wrapped(64, 7, SVIL_PREFIX, &payload);
        assert_eq!(msg.len(), 64);
        assert_eq!(msg[63], 57);
        assert_eq!(payload_capacity(64, VIA_BUFFER_HEADER_BYTES), 54);
        assert_eq!(payload_capacity(32, VIA_BUFFER_HEADER_BYTES), 22);
    }

    #[test]
    fn bootstrap_replies() {
        let nonce = [7u8; NONCE_SIZE];
        let req = build_bootstrap(32, &nonce);
        let mut resp = req.clone();
        resp[25..29].copy_from_slice(&0xCAFE_u32.to_le_bytes());
        resp[29..31].copy_from_slice(&120u16.to_le_bytes());
        assert_eq!(
            parse_bootstrap_reply(&resp, &req),
            BootstrapReply::Granted { client_id: 0xCAFE, ttl_secs: 120 }
        );
        assert_eq!(parse_bootstrap_reply(&req, &req), BootstrapReply::Echoed);
        let mut unhandled = vec![0xFF];
        unhandled.extend_from_slice(&req[1..]);
        assert_eq!(parse_bootstrap_reply(&unhandled, &req), BootstrapReply::Unhandled);
        let mut other = resp.clone();
        other[6] ^= 1;
        assert_eq!(parse_bootstrap_reply(&other, &req), BootstrapReply::Other);
        let mut zero = req.clone();
        zero[29] = 5;
        assert_eq!(parse_bootstrap_reply(&zero, &req), BootstrapReply::ZeroId);
        assert_eq!(lease_millis(120), 50_000.0);
        assert_eq!(lease_millis(30), 27_000.0);
    }

    #[test]
    fn get_info() {
        let mut p = vec![0u8; 26];
        p[0] = 0x00;
        p[1..5].copy_from_slice(&3u32.to_le_bytes());
        p[5..13].copy_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
        p[13] = FLAG_ONESHOT | FLAG_LEADER;
        p[14..17].copy_from_slice(&[0, 0, 9]);
        p[17] = 0x02;
        p[18] = FLAG2_DEFAULT_LAYER_SET;
        let info = SvilInfo::parse(&p).unwrap();
        assert_eq!(info.protocol_version, 3);
        assert_eq!(info.uid, 0x0102_0304_0506_0708);
        assert!(info.has_oneshot() && info.has_leader());
        assert_eq!(info.keycode_version_string(), "0.0.9");
        assert!(info.storage_write_failed() && !info.storage_reset());
        assert!(info.check_supported().is_ok());
        p[1] = 2;
        assert!(SvilInfo::parse(&p).unwrap().check_supported().is_err());
    }

    #[test]
    fn definition_chunks() {
        assert_eq!(definition_chunk_payload(0x0102, 22), vec![0x0E, 0x02, 0x01, 22]);
        let reply = [0x0E, 0, 0, 3, 9, 8, 7, 6, 5];
        assert_eq!(parse_definition_chunk(&reply, 22), &[9, 8, 7]);
        assert_eq!(parse_definition_chunk(&reply, 2), &[9, 8]);
        assert!(check_lzma_header(&[0x5D, 0, 0, 0x10, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_ok());
        assert!(check_lzma_header(&[0xFD; 13]).is_err());
    }

    #[test]
    fn qmk_settings() {
        let reply = [0x10, 1, 0, 2, 0, 0x2A, 0, 0xFF, 0xFF, 3, 0];
        assert_eq!(parse_qmk_settings_query(&reply), vec![1, 2, 42]);
        assert_eq!(parse_qmk_settings_get(&[0x11, 0, 0x34, 0x12, 9], 2), (0, vec![0x34, 0x12]));
        assert_eq!(qmk_settings_set_payload(0x0102, &[5]), vec![0x12, 2, 1, 5, 0, 0, 0]);
    }

    #[test]
    fn labels() {
        assert_eq!(encode_label("héllo")[..6], *"héllo".as_bytes());
        let long = "ééééééééé"; // 18 bytes
        let enc = encode_label(long);
        assert_eq!(decode_label(&enc), "éééééééé");
        assert!(validate_label(long).is_err());
        let mut reply = vec![0x1B, 0, 1, 4, 0];
        reply.extend_from_slice(&encode_label("Nav"));
        assert_eq!(parse_label_scan(&reply), Some((4, "Nav".to_string())));
        assert_eq!(parse_label_scan(&[0x1B, 0, 0, 0, 0]), None);
        assert_eq!(label_set_payload(0, 3, ""), vec![0x1D, 0, 3, 0]);
        assert_eq!(label_set_payload(0, 3, "A")[..5], [0x1C, 0, 3, 0, b'A']);
    }

    #[test]
    fn tables() {
        let mut reply = vec![0x21, 1, 1, 2, 0];
        reply.extend_from_slice(&[4, 0, 5, 0, 0, 0, 0, 0, 6, 0, 0x32, 0x80]);
        let (idx, entry) = parse_table_scan(&reply, &TABLE_COMBO).unwrap();
        assert_eq!(idx, 2);
        let combo = Combo::from_bytes(entry);
        assert_eq!(combo.keys, [4, 5, 0, 0]);
        assert_eq!(combo.output, 6);
        assert_eq!(combo.options, 0x8032);
        let td = TapDance::from_bytes(&[4, 0, 5, 0, 0, 0, 0, 0, 0xC8, 0x80]);
        assert!(td.enabled);
        assert_eq!(td.tapping_term, 200);
    }

    #[test]
    fn keymap_buffers() {
        assert_eq!(keymap_get_buffer_payload(0x0102, 22), vec![0x12, 1, 2, 22]);
        assert_eq!(keymap_set_buffer_payload(4, &[0x0004, 0x5221]), vec![0x13, 0, 4, 4, 0, 4, 0x52, 0x21]);
    }

    #[test]
    fn matrix_reversed_rows() {
        // 2 rows x 10 cols -> 2 bytes per row, MSB first.
        let payload = [0x02, 0x03, 0x00, 0b0000_0010, 0b0000_0001, 0x00, 0x80];
        let m = parse_matrix_state(&payload, 2, 10);
        assert!(m[0][0]);
        assert!(m[0][9]);
        assert!(!m[0][1]);
        assert!(m[1][7]);
    }

    #[test]
    fn entries_roundtrip_and_set_payloads() {
        let td = TapDance { on_tap: 4, on_hold: 0xE1, on_double_tap: 5, on_tap_hold: 6, tapping_term: 200, enabled: true };
        assert_eq!(TapDance::from_bytes(&td.to_bytes()), td);
        let c = Combo { keys: [4, 5, 6, 0], output: 7, options: 0x8032 };
        assert_eq!(Combo::from_bytes(&c.to_bytes()), c);
        assert!(c.enabled() && c.term() == 50);
        let k = KeyOverride { trigger: 4, replacement: 5, layers: 0x0001_FFFF, trigger_mods: 1, negative_mod_mask: 2, suppressed_mods: 3, options: 0x87 };
        assert_eq!(KeyOverride::from_bytes(&k.to_bytes()), k);
        let a = AltRepeatKey { keycode: 0x0204, alt_keycode: 5, allowed_mods: 1, options: 2 };
        assert_eq!(AltRepeatKey::from_bytes(&a.to_bytes()), a);
        let l = Leader { sequence: [4, 5, 6, 0, 0], output: 0x28, options: 1 };
        assert_eq!(Leader::from_bytes(&l.to_bytes()), l);

        let p = tap_dance_set_payload(0x0102, &td);
        assert_eq!(&p[..3], &[0x02, 0x02, 0x01]);
        assert_eq!(&p[3..], &td.to_bytes());
        assert_eq!(combo_set_payload(1, &c)[0], 0x04);
        assert_eq!(key_override_set_payload(1, &k)[0], 0x06);
        assert_eq!(alt_repeat_key_set_payload(1, &a)[0], 0x08);
        assert_eq!(leader_set_payload(1, &l)[0], 0x15);
        assert_eq!(one_shot_set_payload(&OneShot { timeout: 3000, tap_toggle: 5 }), vec![0x0A, 0xB8, 0x0B, 5]);
        assert_eq!(table_get_payload(&TABLE_LEADER, 2), vec![0x14, 2, 0]);
        let mut get_reply = vec![0x14, 2, 0];
        get_reply.extend_from_slice(&l.to_bytes());
        assert_eq!(Leader::from_bytes(parse_table_get(&get_reply, &TABLE_LEADER)), l);
        assert!(check_status(SvilCommand::ComboSet, &[0x04, 0]).is_ok());
        assert!(check_status(SvilCommand::ComboSet, &[0x04, 1]).is_err());
    }

    #[test]
    fn macro_buffer_writes() {
        let buf = [b'a', 0, b'b', b'c', 0, 0, 0, 0];
        let p = macro_buffer_set_payloads(&buf, 2, 32);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], vec![0x20, 0, 0, 0, 0, 5, b'a', 0, b'b', b'c', 0]);
        let big = vec![1u8; 50];
        let p = macro_buffer_set_payloads(&big, 1, 32);
        assert_eq!(p.len(), 3); // 20-byte chunks at 32-byte reports
        assert_eq!(&p[1][1..6], &[20, 0, 0, 0, 20]);
    }

    #[test]
    fn layer_state() {
        let reply = [0x16, 0b1010, 0, 0, 0, 1, 0, 0, 0];
        let s = LayerState::parse(&reply, true);
        assert_eq!(s.active, 0b1010);
        assert_eq!(s.top_layer(), 3);
        assert_eq!(s.default_layer(), Some(0));
        assert_eq!(LayerState::parse(&reply, false).default, None);
        assert_eq!(layer_state_set_payload(0x10), vec![0x17, 0x10, 0, 0, 0]);
        assert_eq!(default_layer_set_payload(2), vec![0x2B, 2]);
        assert_eq!(set_keycode_payload(1, 2, 3, 0x5221), vec![0x05, 1, 2, 3, 0x52, 0x21]);
        assert_eq!(parse_custom_get(&[0x08, 0, 5, 0x20, 0x03, 9], 0, 5, 2), Some(vec![0x20, 0x03]));
        assert_eq!(parse_custom_get(&[0x08, 0, 6, 0x20], 0, 5, 1), None);
        assert_eq!(keymap_block_keys(32), 11);
        assert_eq!(keymap_block_keys(64), 27);
    }

    #[test]
    fn macro_buffer() {
        assert_eq!(macro_buffer_get_payload(0x10000, 20), vec![0x1F, 0, 0, 1, 0, 20]);
        let reply = [0x1F, 0, 0, 1, 0, 2, 0x41, 0x42, 0];
        assert_eq!(parse_macro_buffer_get(&reply, 0x10000, 2).unwrap(), &[0x41, 0x42]);
        assert!(parse_macro_buffer_get(&[0x1F, 1, 0, 0, 0, 0], 0x10000, 2).is_err());
    }
}
