//! Vial keyboard layout manager — WebHID-based live configuration.

use lzma_rs::xz_decompress;
use std::io::Cursor;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

pub mod popup;
pub mod settings;
pub mod webhid;
use popup::VialKeyBindingPopup;

use settings::QmkSettingsPanel;
use vial_protocol::keycodes::keycode_display;
use vial_protocol::qmk_settings as qs;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum VialTab {
    Keymap,
    TestMatrix,
    QmkSettings,
}

/// Raw value bytes for one qsid, keyed by qsid.
#[derive(Clone, PartialEq)]
pub struct QmkSettingValue {
    pub qsid: u16,
    pub value: Vec<u8>,
}

#[derive(Clone, PartialEq, Default)]
struct MatrixPos {
    row: u8,
    col: u8,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

// ---------------------------------------------------------------------------
// VialHome — top-level page component
// ---------------------------------------------------------------------------

#[function_component]
pub fn VialHome() -> Html {
    let connected = use_state(|| false);
    let device = use_state(|| Option::<webhid::HidDevice>::None);
    let keyboard_name = use_state(String::new);
    let vendor_id = use_state(|| 0u16);
    let product_id = use_state(|| 0u16);
    let microcontroller = use_state(String::new);
    let active_tab = use_state(|| VialTab::Keymap);
    let layers = use_state(Vec::<Vec<u16>>::new);
    let layer_count = use_state(|| 0u8);
    let active_layer = use_state(|| 0u8);
    let matrix_rows = use_state(|| 0u8);
    let matrix_cols = use_state(|| 0u8);
    let error = use_state(|| Option::<String>::None);
    let loading = use_state(|| false);
    let matrix_state = use_state(Vec::<Vec<bool>>::new);
    let qmk_settings = use_state(Vec::<QmkSettingValue>::new);
    let definition_info = use_state(|| Option::<String>::None);
    let vial_protocol_ver = use_state(|| 0u32);
    let key_layout = use_state(Vec::<MatrixPos>::new);
    let jump_mode_active = use_state(|| false);
    let jump_input = use_state(|| String::new());
    let selected_key = use_state(|| None::<(usize, usize)>);
    let container_ref = use_node_ref();

    {
        let container_ref = container_ref.clone();
        let connected_state = *connected;
        use_effect_with(connected_state, move |&_is_conn| {
            if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
            || ()
        });
    }

    // -- connect ------------------------------------------------------------
    let on_connect = {
        let connected = connected.clone();
        let device = device.clone();
        let keyboard_name = keyboard_name.clone();
        let vendor_id = vendor_id.clone();
        let product_id = product_id.clone();
        let microcontroller = microcontroller.clone();
        let error = error.clone();
        let loading = loading.clone();
        let layer_count = layer_count.clone();
        let definition_info = definition_info.clone();
        let qmk_settings = qmk_settings.clone();
        let vial_protocol_ver = vial_protocol_ver.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let key_layout = key_layout.clone();
        let layers = layers.clone();

        Callback::from(move |_: MouseEvent| {
            let connected = connected.clone();
            let device = device.clone();
            let keyboard_name = keyboard_name.clone();
            let vendor_id = vendor_id.clone();
            let product_id = product_id.clone();
            let microcontroller = microcontroller.clone();
            let error = error.clone();
            let loading = loading.clone();
            let layer_count = layer_count.clone();
            let definition_info = definition_info.clone();
            let qmk_settings = qmk_settings.clone();
            let vial_protocol_ver = vial_protocol_ver.clone();
            let matrix_rows = matrix_rows.clone();
            let matrix_cols = matrix_cols.clone();
            let key_layout = key_layout.clone();
            let layers = layers.clone();

            spawn_local(async move {
                loading.set(true);
                error.set(None);

                let dev = match webhid::request_device().await {
                    Ok(d) => d,
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                        return;
                    }
                };

                keyboard_name.set(dev.product_name());
                vendor_id.set(dev.vendor_id());
                product_id.set(dev.product_id());

                // 1. Keyboard ID: [vial_proto LE32][uid LE64]
                let mut vial_ver = 0u32;
                if let Ok(resp) = webhid::send_message(
                    &dev,
                    vial_protocol::VialMessage::get_keyboard_id().as_bytes(),
                )
                .await
                {
                    let (ver, uid) = vial_protocol::parse_keyboard_id(&resp);
                    vial_ver = ver;
                    vial_protocol_ver.set(ver);
                    log::info!("Vial protocol v{ver}, UID {uid:016X}");
                }

                // 2. Definition size, then definition blocks
                let mut m_rows = 0u8;
                let mut m_cols = 0u8;
                if let Ok(size_resp) =
                    webhid::send_message(&dev, vial_protocol::VialMessage::get_size().as_bytes())
                        .await
                {
                    let total_size = vial_protocol::parse_definition_size(&size_resp);
                    log::info!("Definition payload: {total_size} bytes");
                    let mut payload = Vec::with_capacity(total_size as usize);
                    let mut remaining = total_size as i64;
                    let mut block = 0u32;
                    while remaining > 0 {
                        match webhid::send_message(
                            &dev,
                            vial_protocol::VialMessage::get_definition(block).as_bytes(),
                        )
                        .await
                        {
                            Ok(resp) => {
                                let chunk_len = if remaining < vial_protocol::MSG_LEN as i64 {
                                    remaining as usize
                                } else {
                                    vial_protocol::MSG_LEN
                                };
                                let chunk = &resp[..chunk_len];

                                payload.extend_from_slice(chunk);
                                remaining -= chunk.len() as i64;
                                block += 1;
                            }
                            Err(e) => {
                                log::warn!("definition fetch error at block {block}: {e}");
                                break;
                            }
                        }
                    }

                    // Decompress XZ
                    let mut decompressed = Vec::new();
                    let mut reader = Cursor::new(&payload);
                    match xz_decompress(&mut reader, &mut decompressed) {
                        Ok(_) => {
                            if let Ok(json) =
                                serde_json::from_slice::<serde_json::Value>(&decompressed)
                            {
                                if let Some(name) = json.get("keyboard_name").and_then(|v| v.as_str()) {
                                    keyboard_name.set(name.to_string());
                                }
                                if let Some(mcu) = json.get("processor").and_then(|v| v.as_str()) {
                                    microcontroller.set(mcu.to_string());
                                } else if let Some(mcu) = json.get("mcu").and_then(|v| v.as_str()) {
                                    microcontroller.set(mcu.to_string());
                                }

                                if let Some(matrix) = json.get("matrix") {
                                    m_rows =
                                        matrix.get("rows").and_then(|v| v.as_u64()).unwrap_or(0)
                                            as u8;
                                    m_cols =
                                        matrix.get("cols").and_then(|v| v.as_u64()).unwrap_or(0)
                                            as u8;
                                    matrix_rows.set(m_rows);
                                    matrix_cols.set(m_cols);
                                }

                                // Parse layout
                                let mut positions = Vec::new();
                                if let Some(layouts) =
                                    json.get("layouts").and_then(|l| l.get("keymap"))
                                {
                                    if let Some(rows) = layouts.as_array() {
                                        let mut cur_y = 0.0f32;
                                        for row_val in rows {
                                            if let Some(keys) = row_val.as_array() {
                                                let mut cur_x = 0.0f32;
                                                let mut cur_w = 1.0f32;
                                                let mut cur_h = 1.0f32;
                                                for k in keys {
                                                    if let Some(obj) = k.as_object() {
                                                        if let Some(x) =
                                                            obj.get("x").and_then(|v| v.as_f64())
                                                        {
                                                            cur_x += x as f32;
                                                        }
                                                        if let Some(y) =
                                                            obj.get("y").and_then(|v| v.as_f64())
                                                        {
                                                            cur_y += y as f32;
                                                        }
                                                        if let Some(w) =
                                                            obj.get("w").and_then(|v| v.as_f64())
                                                        {
                                                            cur_w = w as f32;
                                                        }
                                                        if let Some(h) =
                                                            obj.get("h").and_then(|v| v.as_f64())
                                                        {
                                                            cur_h = h as f32;
                                                        }
                                                    } else if let Some(label) = k.as_str() {
                                                        // Format: "row,col" or "label\n\nrow,col"
                                                        let parts: Vec<&str> =
                                                            label.split('\n').collect();
                                                        let coord = parts.last().unwrap_or(&"");
                                                        let coords: Vec<&str> =
                                                            coord.split(',').collect();
                                                        if coords.len() == 2 {
                                                            let r: u8 =
                                                                coords[0].parse().unwrap_or(255);
                                                            let c: u8 =
                                                                coords[1].parse().unwrap_or(255);
                                                            if r != 255 && c != 255 {
                                                                positions.push(MatrixPos {
                                                                    row: r,
                                                                    col: c,
                                                                    x: cur_x,
                                                                    y: cur_y,
                                                                    w: cur_w,
                                                                    h: cur_h,
                                                                });
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
                                    }
                                }
                                key_layout.set(positions);
                                definition_info.set(Some(format!(
                                    "{} bytes LZMA payload, {} decompressed. Matrix: {}x{}",
                                    payload.len(),
                                    decompressed.len(),
                                    m_rows,
                                    m_cols
                                )));
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("XZ decompression failed: {e}")));
                        }
                    }
                }

                // 3. Layer count
                let mut l_count = 0u8;
                if let Ok(resp) = webhid::send_message(
                    &dev,
                    vial_protocol::VialMessage::get_layer_count().as_bytes(),
                )
                .await
                {
                    l_count = vial_protocol::parse_layer_count(&resp);
                    layer_count.set(l_count);
                    log::info!("layers: {l_count}");
                }

                // 4. Fetch Keymap layer-by-layer
                if l_count > 0 && m_rows > 0 && m_cols > 0 {
                    let mut all_layers = Vec::new();
                    for l in 0..l_count {
                        let mut layer_codes = Vec::new();
                        for r in 0..m_rows {
                            for c in 0..m_cols {
                                if let Ok(resp) = webhid::send_message(
                                    &dev,
                                    vial_protocol::VialMessage::get_keycode(l, r, c).as_bytes(),
                                )
                                .await
                                {
                                    layer_codes.push(vial_protocol::parse_keycode(&resp));
                                } else {
                                    layer_codes.push(0);
                                }
                            }
                        }
                        all_layers.push(layer_codes);
                        layers.set(all_layers.clone());
                    }
                }

                // 5. QMK settings (only if vial protocol >= 4)
                if vial_ver >= vial_protocol::VIAL_PROTOCOL_QMK_SETTINGS {
                    let settings = load_qmk_settings(&dev).await;
                    qmk_settings.set(settings);
                }

                device.set(Some(dev));
                connected.set(true);
                loading.set(false);
            });
        })
    };

    // -- disconnect ---------------------------------------------------------
    let on_disconnect = {
        let connected = connected.clone();
        let device = device.clone();
        let keyboard_name = keyboard_name.clone();
        let vendor_id = vendor_id.clone();
        let product_id = product_id.clone();
        let microcontroller = microcontroller.clone();
        let layers = layers.clone();
        let layer_count = layer_count.clone();
        let active_layer = active_layer.clone();
        let qmk_settings = qmk_settings.clone();

        Callback::from(move |_: MouseEvent| {
            let device = device.clone();
            let connected = connected.clone();
            let keyboard_name = keyboard_name.clone();
            let vendor_id = vendor_id.clone();
            let product_id = product_id.clone();
            let microcontroller = microcontroller.clone();
            let layers = layers.clone();
            let layer_count = layer_count.clone();
            let active_layer = active_layer.clone();
            let qmk_settings = qmk_settings.clone();

            if let Some(dev) = (*device).clone() {
                spawn_local(async move {
                    let _ = webhid::close_device(&dev).await;
                    device.set(None);
                    connected.set(false);
                    keyboard_name.set(String::new());
                    vendor_id.set(0);
                    product_id.set(0);
                    microcontroller.set(String::new());
                    layers.set(Vec::new());
                    layer_count.set(0);
                    active_layer.set(0);
                    qmk_settings.set(Vec::new());
                });
            }
        })
    };

    // -- tab switchers ------------------------------------------------------

    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let mut hint_map = std::collections::HashMap::new();
    let mut hint_idx = 0;

    // Layer hints (2 chars)
    let layer_count_val = *layer_count;
    let mut layer_hints = vec![String::new(); layer_count_val as usize];
    for i in 0..layer_count_val as usize {
        if hint_idx < hint_chars.len() * hint_chars.len() {
            let h = format!(
                "{}{}",
                hint_chars.chars().nth(hint_idx / hint_chars.len()).unwrap(),
                hint_chars.chars().nth(hint_idx % hint_chars.len()).unwrap()
            );
            layer_hints[i] = h.clone();
            hint_map.insert(h, (None, Some(i)));
            hint_idx += 1;
        }
    }

    // Key hints (2 chars)
    let mut key_hints = vec![String::new(); (*key_layout).len()];
    for (i, _) in (*key_layout).iter().enumerate() {
        if hint_idx < hint_chars.len() * hint_chars.len() {
            let h = format!(
                "{}{}",
                hint_chars.chars().nth(hint_idx / hint_chars.len()).unwrap(),
                hint_chars.chars().nth(hint_idx % hint_chars.len()).unwrap()
            );
            key_hints[i] = h.clone();
            hint_map.insert(h, (Some(i), None));
            hint_idx += 1;
        }
    }

    let on_keydown = {
        let jump_mode_active = jump_mode_active.clone();
        let jump_input = jump_input.clone();
        let selected_key = selected_key.clone();
        let hint_map = hint_map.clone();
        let active_layer = active_layer.clone();
        Callback::from(move |e: KeyboardEvent| {
            if selected_key.is_some() {
                return;
            }

            if *jump_mode_active {
                match e.key().as_str() {
                    "Enter" | "Escape" => {
                        jump_mode_active.set(false);
                        jump_input.set(String::new());
                        e.prevent_default();
                    }
                    key if key.len() == 1 && hint_chars.contains(key) => {
                        let mut new_input = (*jump_input).clone();
                        new_input.push_str(key);
                        if let Some(&(key_idx, layer_idx)) = hint_map.get(&new_input) {
                            if let Some(idx) = key_idx {
                                selected_key.set(Some((*active_layer as usize, idx)));
                            } else if let Some(l_idx) = layer_idx {
                                active_layer.set(l_idx as u8);
                            }
                            jump_mode_active.set(false);
                            jump_input.set(String::new());
                        } else if hint_map.keys().any(|h| h.starts_with(&new_input)) {
                            jump_input.set(new_input);
                        }
                        e.prevent_default();
                    }
                    _ => {}
                }
            } else if e.key() == "j" {
                jump_mode_active.set(true);
                jump_input.set(String::new());
                e.prevent_default();
            }
        })
    };

    let on_key_click = {
        let selected_key = selected_key.clone();
        let active_layer = active_layer.clone();
        Callback::from(move |idx: usize| {
            selected_key.set(Some((*active_layer as usize, idx)));
        })
    };

    let on_save = {
        let layers = layers.clone();
        let device = device.clone();
        let matrix_cols = matrix_cols.clone();
        Callback::from(move |_: MouseEvent| {
            let dev = device.clone();
            let lays = layers.clone();
            let cols = *matrix_cols;
            spawn_local(async move {
                if let Some(d) = &*dev {
                    for (l_idx, layer) in lays.iter().enumerate() {
                        for (k_idx, &kc) in layer.iter().enumerate() {
                            let r = (k_idx / cols as usize) as u8;
                            let c = (k_idx % cols as usize) as u8;
                            let msg =
                                vial_protocol::VialMessage::set_keycode(l_idx as u8, r, c, kc);
                            let _ = webhid::send_message(d, msg.as_bytes()).await;
                        }
                    }
                }
            });
        })
    };

    let set_tab = |tab: VialTab| {
        let active_tab = active_tab.clone();
        Callback::from(move |_: MouseEvent| active_tab.set(tab.clone()))
    };

    // -- QMK settings callbacks ---------------------------------------------
    let on_setting_change = {
        let device = device.clone();
        let error = error.clone();
        let qmk_settings = qmk_settings.clone();
        Callback::from(move |(qsid, value): (u16, Vec<u8>)| {
            let device = device.clone();
            let error = error.clone();
            let qmk_settings = qmk_settings.clone();
            spawn_local(async move {
                if let Some(dev) = (*device).as_ref() {
                    let msg = vial_protocol::VialMessage::qmk_settings_set(qsid, &value);
                    match webhid::send_message(dev, msg.as_bytes()).await {
                        Ok(_) => {
                            let mut s = (*qmk_settings).clone();
                            if let Some(entry) = s.iter_mut().find(|e| e.qsid == qsid) {
                                entry.value = value;
                            }
                            qmk_settings.set(s);
                        }
                        Err(e) => error.set(Some(format!("Failed to set setting: {e}"))),
                    }
                }
            });
        })
    };

    let on_settings_reset = {
        let device = device.clone();
        let error = error.clone();
        Callback::from(move |_: ()| {
            let device = device.clone();
            let error = error.clone();
            spawn_local(async move {
                if let Some(dev) = (*device).as_ref() {
                    let msg = vial_protocol::VialMessage::qmk_settings_reset();
                    if let Err(e) = webhid::send_message(dev, msg.as_bytes()).await {
                        error.set(Some(format!("Failed to reset: {e}")));
                    }
                }
            });
        })
    };

    // -- helpers ------------------------------------------------------------
    let tab_class = |tab: &VialTab| -> &'static str {
        if *active_tab == *tab {
            "px-4 py-2 border-b-2 border-blue-500 text-blue-500 font-medium"
        } else {
            "px-4 py-2 border-b-2 border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        }
    };

    // -- render -------------------------------------------------------------
    let def_sidebar = if let Some(info) = &*definition_info {
        html! {
            <div class="w-full lg:w-64 shrink-0 p-5 bg-gray-50 dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm h-fit">
                <h3 class="text-xs font-bold mb-4 flex items-center gap-2 text-gray-400 uppercase tracking-widest border-b border-gray-200 dark:border-gray-700 pb-2">
                    <span class="w-1.5 h-1.5 bg-blue-500 rounded-full"></span>
                    {"Device Info"}
                </h3>
                <div class="grid grid-cols-2 gap-x-4 gap-y-4">
                    <div class="space-y-1">
                        <div class="text-[10px] font-bold text-gray-400 uppercase tracking-widest">{"Name"}</div>
                        <div class="text-xs font-medium text-gray-900 dark:text-gray-100 truncate" title={keyboard_name.to_string()}>{&*keyboard_name}</div>
                    </div>
                    <div class="space-y-1">
                        <div class="text-[10px] font-bold text-gray-400 uppercase tracking-widest">{"MCU"}</div>
                        <div class="text-xs font-medium text-gray-900 dark:text-gray-100 truncate" title={microcontroller.to_string()}>
                            {if microcontroller.is_empty() { "Unknown" } else { &*microcontroller }}
                        </div>
                    </div>
                    <div class="space-y-1">
                        <div class="text-[10px] font-bold text-gray-400 uppercase tracking-widest">{"VID"}</div>
                        <div class="text-xs font-mono text-gray-900 dark:text-gray-100">{format!("0x{:04X}", *vendor_id)}</div>
                    </div>
                    <div class="space-y-1">
                        <div class="text-[10px] font-bold text-gray-400 uppercase tracking-widest">{"PID"}</div>
                        <div class="text-xs font-mono text-gray-900 dark:text-gray-100">{format!("0x{:04X}", *product_id)}</div>
                    </div>
                </div>
                <div class="mt-5 pt-4 border-t border-gray-200 dark:border-gray-700">
                    <div class="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-1">{"Matrix & Payload"}</div>
                    <p class="text-[10px] text-gray-500 font-mono leading-relaxed">{info}</p>
                </div>
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <div ref={container_ref.clone()} class="w-full max-w-[90rem] mx-auto py-4 outline-none" tabindex="0" onkeydown={on_keydown}>
            <div class="flex flex-col lg:flex-row gap-8 items-start">
                if *connected && definition_info.is_some() {
                    {def_sidebar}
                }

                <div class="flex-1 min-w-0 w-full">
                    // Connection bar
                    <div class="flex items-center gap-4 mb-6 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
                        if *connected {
                            <div class="w-3 h-3 bg-green-500 rounded-full"></div>
                            <span class="font-medium">{&*keyboard_name}</span>
                            <span class="text-sm text-gray-500">{format!("{} layers", *layer_count)}</span>
                            <button onclick={on_save.clone()} class="ml-auto bg-green-600 hover:bg-green-700 text-white font-bold py-1.5 px-4 rounded shadow text-sm">
                                {"Save Keymap to Device"}
                            </button>
                            <button
                                class="px-4 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600"
                                onclick={on_disconnect}
                            >
                                {"Disconnect"}
                            </button>
                        } else {
                            <div class="w-3 h-3 bg-gray-400 rounded-full"></div>
                            <span class="text-gray-500">{"No device connected"}</span>
                            <button
                                class="ml-auto px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                                onclick={on_connect}
                                disabled={*loading}
                            >
                                { if *loading { "Connecting..." } else { "Connect Keyboard" } }
                            </button>
                        }
                    </div>

                    // Error
                    if let Some(err) = &*error {
                        <div class="mb-4 p-3 bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-200 rounded">
                            {err}
                            <button
                                class="ml-2 underline"
                                onclick={let error = error.clone(); Callback::from(move |_: MouseEvent| error.set(None))}
                            >
                                {"dismiss"}
                            </button>
                        </div>
                    }

                    if *connected {
                        // Tab bar
                        <div class="flex border-b border-gray-200 dark:border-gray-700 mb-4">
                            <button class={tab_class(&VialTab::Keymap)} onclick={set_tab(VialTab::Keymap)}>{"Keymap"}</button>
                            <button class={tab_class(&VialTab::TestMatrix)} onclick={set_tab(VialTab::TestMatrix)}>{"Test Matrix"}</button>
                            <button class={tab_class(&VialTab::QmkSettings)} onclick={set_tab(VialTab::QmkSettings)}>{"QMK Settings"}</button>
                        </div>


                    { if let Some((l_idx, k_idx)) = *selected_key {
                        let current_keycode = layers.get(l_idx).and_then(|l| l.get(k_idx)).copied().unwrap_or(0);
                        let on_close = {
                            let selected_key = selected_key.clone();
                            let container_ref = container_ref.clone();
                            Callback::from(move |_: MouseEvent| {
                                selected_key.set(None);
                                if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                                    let _ = element.focus();
                                }
                            })
                        };
                        let on_apply = {
                            let selected_key = selected_key.clone();
                            let layers = layers.clone();
                            Callback::from(move |kc: u16| {
                                let mut new_layers = (*layers).clone();
                                new_layers[l_idx][k_idx] = kc;
                                layers.set(new_layers);
                                selected_key.set(None);

                            })
                        };
                        html! { <VialKeyBindingPopup current_keycode={current_keycode} protocol_version={*vial_protocol_ver} keyboard_name={(*keyboard_name).clone()} on_close={on_close} on_apply={on_apply} /> }
                    } else { html! {} } }

                        // Tab content
                        { match &*active_tab {
                            VialTab::Keymap => render_keymap_tab(
                                &layers, *layer_count, &active_layer, &key_layout, *matrix_cols,
                                on_key_click.clone(), *jump_mode_active, &*jump_input, &key_hints, &layer_hints,
                                *vial_protocol_ver,
                                &*keyboard_name
                            ),
                            VialTab::TestMatrix => render_matrix_tab(&matrix_state, *matrix_cols),
                            VialTab::QmkSettings => html! {
                                <QmkSettingsPanel
                                    settings={(*qmk_settings).clone()}
                                    on_change={on_setting_change}
                                    on_reset={on_settings_reset}
                                    loading={*loading}
                                />
                            },
                        }}
                    } else {
                        <div class="text-center py-20 text-gray-500">
                            <p class="text-2xl mb-4">{"Vial Keyboard Manager"}</p>
                            <p>{"Connect a Vial-compatible keyboard via USB to get started."}</p>
                            <p class="text-sm mt-2">{"Requires a Chromium-based browser (Chrome, Edge) with WebHID support."}</p>
                        </div>
                    }
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// QMK settings loader — queries supported qsids then reads their values
// ---------------------------------------------------------------------------

async fn load_qmk_settings(dev: &webhid::HidDevice) -> Vec<QmkSettingValue> {
    // Step 1: discover all supported qsids via cursor-based pagination
    let mut supported: Vec<u16> = Vec::new();
    let mut cursor: u16 = 0;
    loop {
        let msg = vial_protocol::VialMessage::qmk_settings_query(cursor);
        let resp = match webhid::send_message(dev, msg.as_bytes()).await {
            Ok(r) => r,
            Err(_) => break,
        };
        let qsids = vial_protocol::parse_qmk_settings_query(&resp);
        if qsids.is_empty() {
            break;
        }
        let mut max_qsid = cursor;
        for &qsid in &qsids {
            if !supported.contains(&qsid) {
                supported.push(qsid);
            }
            max_qsid = max_qsid.max(qsid);
        }
        // Advance cursor past the largest qsid we saw
        if max_qsid == cursor {
            break; // no progress
        }
        cursor = max_qsid;
    }

    log::info!(
        "QMK settings: {} supported qsids: {:?}",
        supported.len(),
        supported
    );

    // Step 2: for each supported qsid that we know about, read its value
    let mut values = Vec::new();
    // Deduplicate: only read each qsid once
    let mut read_qsids = Vec::new();
    for &qsid in &supported {
        if read_qsids.contains(&qsid) {
            continue;
        }
        // Only read if we have a definition for it
        if !qs::is_qsid_supported(qsid) {
            continue;
        }
        let width = qs::width_for_qsid(qsid) as usize;
        let msg = vial_protocol::VialMessage::qmk_settings_get(qsid);
        match webhid::send_message(dev, msg.as_bytes()).await {
            Ok(resp) => {
                let (_status, val) = vial_protocol::parse_qmk_settings_get(&resp, width);
                values.push(QmkSettingValue { qsid, value: val });
            }
            Err(e) => {
                log::warn!("Failed to read qsid {qsid}: {e}");
            }
        }
        read_qsids.push(qsid);
    }

    values
}

// ---------------------------------------------------------------------------
// Sub-renders
// ---------------------------------------------------------------------------


fn render_keymap_tab(
    layers: &UseStateHandle<Vec<Vec<u16>>>,
    layer_count: u8,
    active_layer: &UseStateHandle<u8>,
    key_layout: &UseStateHandle<Vec<MatrixPos>>,
    matrix_cols: u8,
    on_key_click: Callback<usize>,
    jump_mode_active: bool,
    jump_input: &str,
    key_hints: &[String],
    layer_hints: &[String],
    protocol_version: u32,
    keyboard_name: &str,
) -> Html {
    let idx = **active_layer as usize;
    let layer_keys = layers.get(idx);

    let layout_html = if let Some(codes) = layer_keys {
        if !key_layout.is_empty() {
            // Render using physical layout
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            let mut avg_w = 0.0;
            for pos in key_layout.iter() {
                avg_w += pos.w;
                if pos.x < min_x { min_x = pos.x; }
                if pos.x + pos.w > max_x { max_x = pos.x + pos.w; }
                if pos.y < min_y { min_y = pos.y; }
                if pos.y + pos.h > max_y { max_y = pos.y + pos.h; }
            }
            if !key_layout.is_empty() {
                avg_w /= key_layout.len() as f32;
            }

            let u_size = if avg_w < 5.0 { 1.0 } else if avg_w < 500.0 { 100.0 } else { 1000.0 };
            let size_scale = 44.0 / u_size;
            let u_pos = if max_x.abs() > 20000.0 || min_x.abs() > 20000.0 {
                19050.0
            } else {
                u_size
            };
            let pos_scale = 44.0 / u_pos;

            let content_width_px = (max_x - min_x) * pos_scale;
            let offset_x = -(min_x * pos_scale);

            html! {
                <div class="relative bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-xl p-8 overflow-auto shadow-inner w-full max-w-full"
                     style="min-height: 350px; height: 55vh;">
                    <div class="relative mx-auto" style={format!("width: {}px;", content_width_px)}>
                        { for key_layout.iter().map(|pos| {
                            let keycode_idx = (pos.row as usize) * (matrix_cols as usize) + (pos.col as usize);
                            let kc = codes.get(keycode_idx).copied().unwrap_or(0);
                            let disp = keycode_display(kc, protocol_version, keyboard_name);
                            let hint = key_hints.get(keycode_idx);
                            let show_hint = jump_mode_active && hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                            let onclick = { let cb = on_key_click.clone(); Callback::from(move |_: MouseEvent| cb.emit(keycode_idx)) };
                            let x = (pos.x * pos_scale + offset_x) as i32;
                            let y = (pos.y * pos_scale) as i32;
                            let w = (pos.w * size_scale).max(20.0) as i32 - 4;
                            let h = (pos.h * size_scale).max(20.0) as i32 - 4;
                            let style = format!(
                                "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px;",
                                x, y, w, h
                            );
                            html! {
                                <div
                                    onclick={onclick}
                                    class="flex flex-col items-center justify-center font-bold border rounded-md
                                           bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600
                                           shadow-sm hover:shadow-md hover:border-blue-400 dark:hover:border-blue-500
                                           transition-all cursor-pointer select-none relative"
                                    style={style}
                                    title={format!("Row {}, Col {}
Keycode: 0x{:04X}", pos.row, pos.col, kc)}
                                >
                                    <div class="absolute top-0.5 left-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%]">{disp.upper_left}</div>
                                    <div class="absolute top-0.5 left-0 right-0 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-center mx-auto">{disp.upper_middle}</div>
                                    <div class="absolute top-0.5 right-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-right">{disp.upper_right}</div>
                                    <div class="text-[9px] leading-tight text-center px-1 break-all">{disp.middle}</div>
                                    { if show_hint {
                                        let h = hint.unwrap();
                                        let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                        html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                    } else { html! {} }}
                                </div>
                            }
                        })}
                    </div>
                </div>
            }

        } else {
            // Fallback: simple grid
            html! {
                <div class="flex flex-wrap gap-1 my-4">
                    { for codes.iter().enumerate().map(|(keycode_idx, &kc)| {
                        let disp = keycode_display(kc, protocol_version, keyboard_name);
                        let hint = key_hints.get(keycode_idx);
                        let show_hint = jump_mode_active && hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                        let onclick = { let cb = on_key_click.clone(); Callback::from(move |_: MouseEvent| cb.emit(keycode_idx)) };
                        html! {
                            <div
                                onclick={onclick}
                                class="w-14 h-14 flex flex-col items-center justify-center border rounded
                                       bg-gray-100 dark:bg-gray-800 border-gray-300 dark:border-gray-600
                                       hover:bg-blue-100 dark:hover:bg-blue-900 cursor-pointer select-none relative"
                                title={format!("0x{kc:04X}")}
                            >
                                <div class="absolute top-0.5 left-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%]">{disp.upper_left}</div>
                                <div class="absolute top-0.5 left-0 right-0 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-center mx-auto">{disp.upper_middle}</div>
                                <div class="absolute top-0.5 right-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-right">{disp.upper_right}</div>
                                <div class="text-[9px] leading-tight text-center px-1 break-all">{disp.middle}</div>
                                { if show_hint {
                                    let h = hint.unwrap();
                                    let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                    html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                } else { html! {} }}
                            </div>
                        }
                    })}
                </div>
            }
        }
    } else {
        html! {
            <div class="py-12 text-center text-gray-400 italic">
                {if layer_count > 0 { "Loading keymap data..." } else { "Connect a keyboard to view the keymap." }}
            </div>
        }
    };

    html! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-4 flex-wrap">
                <div class="flex items-center space-x-4">
                    <div>
                        <h2 class="text-xl font-bold">{"Keymap Editor"}</h2>
                        <p class="text-sm text-gray-500">
                            {"Live configuration of your keyboard layers."}
                        </p>
                        <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                            {"Type "} <kbd class="px-1.5 py-0.5 font-sans font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">{"j"}</kbd> {" to start jump mode"}
                        </p>
                    </div>
                </div>

                if layer_count > 0 {
                    <div class="flex p-1 bg-gray-100 dark:bg-gray-800 rounded-lg shadow-sm">
                        { for (0..layer_count).map(|i| {
                            let active_layer = active_layer.clone();
                            let is_active = *active_layer == i;
                            let hint = layer_hints.get(i as usize);
                            let show_hint = jump_mode_active && hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                            html! {
                                <button
                                    class={classes!("px-4", "py-1.5", "rounded-md", "shadow-sm", "font-medium", "transition-all", "relative",
                                        if is_active { "bg-white dark:bg-gray-700 text-blue-600 dark:text-blue-400" } else { "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300" }
                                    )}
                                    onclick={Callback::from(move |_: MouseEvent| active_layer.set(i))}
                                >
                                    {format!("L{}", i)}
                                    { if show_hint {
                                        let h = hint.unwrap();
                                        let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                        html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                    } else { html! {} }}
                                </button>
                            }
                        })}
                    </div>
                }
            </div>

            {layout_html}
        </div>
    }
}

fn render_matrix_tab(matrix_state: &UseStateHandle<Vec<Vec<bool>>>, cols: u8) -> Html {
    html! {
        <div>
            <p class="text-gray-500 mb-4">
                {"Press keys on your keyboard to test the switch matrix."}
            </p>
            if cols > 0 {
                <div
                    class="grid gap-1"
                    style={format!("grid-template-columns: repeat({cols}, 2.5rem)")}
                >
                    { for matrix_state.iter().flat_map(|row| {
                        row.iter().map(|&pressed| {
                            html! {
                                <div class={if pressed {
                                    "w-10 h-10 bg-green-500 rounded border border-green-600"
                                } else {
                                    "w-10 h-10 bg-gray-200 dark:bg-gray-700 rounded border border-gray-300 dark:border-gray-600"
                                }}></div>
                            }
                        })
                    })}
                </div>
            } else {
                <p class="text-gray-400">{"Matrix size unknown \u{2014} definition not yet decoded."}</p>
            }
        </div>
    }
}
