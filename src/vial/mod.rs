//! Vial keyboard layout manager — WebHID-based live configuration.

use lzma_rs::xz_decompress;
use std::io::Cursor;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
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

#[derive(Clone, PartialEq, Debug)]
enum HintTarget {
    Layer(usize),
    LayerMenu(usize),
    Menu(usize, usize),
    Key(usize),
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

    // Layer menu state
    let layer_menu_index = use_state(|| None::<usize>);
    let menu_focus_index = use_state(|| 0usize);
    let quick_assign_index = use_state(|| None::<usize>);
    let layer_names = use_state(|| Vec::<String>::new());

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
        let layer_names = layer_names.clone();

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
            let layer_names = layer_names.clone();

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
                    // Initialize layer names
                    let names: Vec<String> = (0..l_count).map(|i| format!("L{}", i)).collect();
                    layer_names.set(names);
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
        let layer_names = layer_names.clone();
        let layer_menu_index = layer_menu_index.clone();

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
            let layer_names = layer_names.clone();
            let layer_menu_index = layer_menu_index.clone();

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
                    layer_names.set(Vec::new());
                    layer_menu_index.set(None);
                });
            }
        })
    };

    // -- tab switchers ------------------------------------------------------

    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let mut hint_map: std::collections::HashMap<String, HintTarget> = std::collections::HashMap::new();
    let _hint_idx = 0;

    // Build all targets: layers, layer menus, menu items (if open), keys
    let mut all_targets = Vec::new();
    let layer_count_val = *layer_count as usize;
    for i in 0..layer_count_val {
        all_targets.push(HintTarget::Layer(i));
        all_targets.push(HintTarget::LayerMenu(i));
        if let Some(lmi) = *layer_menu_index {
            if lmi == i {
                for j in 0..9 {
                    all_targets.push(HintTarget::Menu(i, j));
                }
            }
        }
    }

    // Key hints
    let key_count = (*key_layout).len();
    for i in 0..key_count {
        all_targets.push(HintTarget::Key(i));
    }

    // Assign hints to targets
    let mut layer_hints = vec![String::new(); layer_count_val];
    let mut key_hints = vec![String::new(); key_count];
    for (i, target) in all_targets.into_iter().enumerate() {
        if i < hint_chars.len() * hint_chars.len() {
            let h = format!(
                "{}{}",
                hint_chars.chars().nth(i / hint_chars.len()).unwrap(),
                hint_chars.chars().nth(i % hint_chars.len()).unwrap()
            );
            match target {
                HintTarget::Layer(idx) => layer_hints[idx] = h.clone(),
                HintTarget::Key(idx) => key_hints[idx] = h.clone(),
                _ => {}
            }
            hint_map.insert(h, target);
        }
    }

    // -- layer menu actions -------------------------------------------------

    let move_layer = {
        let layers = layers.clone();
        let active_layer = active_layer.clone();
        let layer_menu_index = layer_menu_index.clone();
        let layer_names = layer_names.clone();
        move |idx: usize, up: bool| {
            let mut lays = (*layers).clone();
            if up && idx == 0 {
                return;
            }
            if !up && idx >= lays.len() - 1 {
                return;
            }
            let target = if up { idx - 1 } else { idx + 1 };
            lays.swap(idx, target);
            // Sync active_layer if it moved
            let current = *active_layer as usize;
            if current == idx {
                active_layer.set(target as u8);
            } else if current == target {
                active_layer.set(idx as u8);
            }
            // Also swap layer names
            let mut names = (*layer_names).clone();
            names.swap(idx, target);
            layer_names.set(names);
            layers.set(lays);
            layer_menu_index.set(None);
        }
    };

    let rename_layer = {
        let layer_names = layer_names.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let window = web_sys::window().unwrap();
            let current_name = &layer_names[idx];
            if let Ok(Some(new_name)) =
                window.prompt_with_message_and_default("Rename layer:", current_name)
            {
                let trimmed = new_name.trim();
                if !trimmed.is_empty() && trimmed.len() <= 32 {
                    let mut names = (*layer_names).clone();
                    names[idx] = trimmed.to_string();
                    layer_names.set(names);
                }
            }
            layer_menu_index.set(None);
        }
    };

    let duplicate_layer = {
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            // Max layers limit check (Vial typically supports 32 layers)
            if layers.len() >= 32 {
                return;
            }
            let mut lays = (*layers).clone();
            let mut names = (*layer_names).clone();
            let new_layer = lays[idx].clone();
            let new_name = format!("{} (copy)", names[idx]);
            lays.insert(idx + 1, new_layer);
            names.insert(idx + 1, new_name);
            layers.set(lays);
            layer_names.set(names);
            layer_menu_index.set(None);
        }
    };

    let delete_layer = {
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let active_layer = active_layer.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            // Prevent deleting last layer
            if layers.len() <= 1 {
                return;
            }
            // Optional: Confirm dialog for destructive action
            let window = web_sys::window().unwrap();
            let confirmed = window
                .confirm_with_message(&format!(
                    "Delete layer '{}'? This cannot be undone.",
                    layer_names[idx]
                ))
                .unwrap_or(false);
            if !confirmed {
                layer_menu_index.set(None);
                return;
            }
            let mut lays = (*layers).clone();
            let mut names = (*layer_names).clone();
            lays.remove(idx);
            names.remove(idx);
            // Adjust active_layer if necessary
            let current = *active_layer as usize;
            if current >= lays.len() {
                active_layer.set((lays.len() - 1) as u8);
            } else if current == idx && current > 0 {
                active_layer.set((current - 1) as u8);
            }
            layers.set(lays);
            layer_names.set(names);
            layer_menu_index.set(None);
        }
    };

    let reset_layer = {
        let layers = layers.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let mut lays = (*layers).clone();
            let key_count = lays[idx].len();
            lays[idx] = vec![0x0000; key_count]; // KC_NO = 0x0000
            layers.set(lays);
            layer_menu_index.set(None);
        }
    };

    let trans_to_none = {
        let layers = layers.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let mut lays = (*layers).clone();
            for kc in lays[idx].iter_mut() {
                if *kc == 0x0001 {
                    // KC_TRANSPARENT
                    *kc = 0x0000; // KC_NO
                }
            }
            layers.set(lays);
            layer_menu_index.set(None);
        }
    };

    let none_to_trans = {
        let layers = layers.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let mut lays = (*layers).clone();
            for kc in lays[idx].iter_mut() {
                if *kc == 0x0000 {
                    // KC_NO
                    *kc = 0x0001; // KC_TRANSPARENT
                }
            }
            layers.set(lays);
            layer_menu_index.set(None);
        }
    };

    let start_quick_assign = {
        let quick_assign_index = quick_assign_index.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |_| {
            quick_assign_index.set(Some(0)); // Start at first key
            layer_menu_index.set(None);
        }
    };

    // -- click outside to close menu ----------------------------------------
    {
        let layer_menu_index = layer_menu_index.clone();
        use_effect(move || {
            let lmi = layer_menu_index.clone();
            let click_listener = Closure::wrap(
                Box::new(move |_e: MouseEvent| lmi.set(None)) as Box<dyn FnMut(MouseEvent)>
            );
            let lmi_esc = layer_menu_index.clone();
            let key_listener = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                if e.key() == "Escape" {
                    lmi_esc.set(None);
                }
            }) as Box<dyn FnMut(KeyboardEvent)>);
            let window = web_sys::window().unwrap();
            window
                .add_event_listener_with_callback("click", click_listener.as_ref().unchecked_ref())
                .unwrap();
            window
                .add_event_listener_with_callback("keydown", key_listener.as_ref().unchecked_ref())
                .unwrap();
            move || {
                let window = web_sys::window().unwrap();
                window
                    .remove_event_listener_with_callback(
                        "click",
                        click_listener.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                window
                    .remove_event_listener_with_callback(
                        "keydown",
                        key_listener.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                drop(click_listener);
                drop(key_listener);
            }
        });
    }

    let on_keydown = {
        let jump_mode_active = jump_mode_active.clone();
        let jump_input = jump_input.clone();
        let selected_key = selected_key.clone();
        let hint_map = hint_map.clone();
        let active_layer = active_layer.clone();
        let layer_menu_index = layer_menu_index.clone();
        let menu_focus_index = menu_focus_index.clone();
        let quick_assign_index = quick_assign_index.clone();
        let layers = layers.clone();
        let key_layout = key_layout.clone();
        let move_l = move_layer.clone();
        let dup_l = duplicate_layer.clone();
        let del_l = delete_layer.clone();
        let ren_l = rename_layer.clone();
        let res_l = reset_layer.clone();
        let batch_t_n = trans_to_none.clone();
        let batch_n_t = none_to_trans.clone();
        let start_qa = start_quick_assign.clone();
        Callback::from(move |e: KeyboardEvent| {
            // Handle menu keyboard navigation when menu is open
            if let Some(l_idx) = *layer_menu_index {
                match e.key().as_str() {
                    "ArrowDown" => {
                        menu_focus_index.set((*menu_focus_index + 1) % 9);
                        e.prevent_default();
                        return;
                    }
                    "ArrowUp" => {
                        menu_focus_index.set((*menu_focus_index + 8) % 9);
                        e.prevent_default();
                        return;
                    }
                    "Enter" => {
                        match *menu_focus_index {
                            0 => move_l(l_idx, true),
                            1 => move_l(l_idx, false),
                            2 => ren_l(l_idx),
                            3 => dup_l(l_idx),
                            4 => del_l(l_idx),
                            5 => res_l(l_idx),
                            6 => batch_t_n(l_idx),
                            7 => batch_n_t(l_idx),
                            8 => start_qa(()),
                            _ => {}
                        }
                        e.prevent_default();
                        return;
                    }
                    "Escape" => {
                        layer_menu_index.set(None);
                        e.prevent_default();
                        return;
                    }
                    _ => {}
                }
            }

            // Quick assign mode handling
            if let Some(idx) = *quick_assign_index {
                if e.key() == "Escape" {
                    quick_assign_index.set(None);
                    e.prevent_default();
                    return;
                }
                // Try to parse as a simple keypress for quick assign
                // For simplicity, we'll just handle single characters and some special keys
                if e.key().len() == 1 {
                    // This is a simplified keycode mapping - just using ASCII for letters
                    // A full implementation would map all keys to proper keycodes
                    let key = e.key();
                    if let Some(first) = key.chars().next() {
                        let kc: u16 = if first.is_ascii_alphabetic() {
                            // Convert to uppercase and use as keycode offset
                            (first.to_ascii_uppercase() as u16) - ('A' as u16) + 0x04
                        } else if first.is_ascii_digit() {
                            if first == '0' {
                                0x27
                            } else {
                                (first as u16) - ('1' as u16) + 0x1e
                            }
                        } else {
                            0x0000 // KC_NO for unsupported keys
                        };
                        if kc != 0x0000 {
                            let mut lays = (*layers).clone();
                            let layer_idx = *active_layer as usize;
                            if layer_idx < lays.len() && idx < lays[layer_idx].len() {
                                lays[layer_idx][idx] = kc;
                                layers.set(lays);
                                // Advance to next key
                                let key_count = (*key_layout).len();
                                if key_count > 0 {
                                    quick_assign_index.set(Some((idx + 1) % key_count));
                                }
                            }
                            e.prevent_default();
                        }
                    }
                }
                return;
            }

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
                        if let Some(target) = hint_map.get(&new_input) {
                            match target {
                                HintTarget::Key(idx) => {
                                    selected_key.set(Some((*active_layer as usize, *idx)));
                                }
                                HintTarget::Layer(idx) => {
                                    active_layer.set(*idx as u8);
                                }
                                HintTarget::LayerMenu(idx) => {
                                    layer_menu_index.set(Some(*idx));
                                    menu_focus_index.set(0);
                                }
                                HintTarget::Menu(l_idx, m_idx) => match *m_idx {
                                    0 => move_l(*l_idx, true),
                                    1 => move_l(*l_idx, false),
                                    2 => ren_l(*l_idx),
                                    3 => dup_l(*l_idx),
                                    4 => del_l(*l_idx),
                                    5 => res_l(*l_idx),
                                    6 => batch_t_n(*l_idx),
                                    7 => batch_n_t(*l_idx),
                                    8 => start_qa(()),
                                    _ => {}
                                },
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
        let quick_assign_index = quick_assign_index.clone();
        Callback::from(move |idx: usize| {
            if quick_assign_index.is_some() {
                quick_assign_index.set(Some(idx));
            } else {
                selected_key.set(Some((*active_layer as usize, idx)));
            }
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

    // -- Save keymap as .vil file with dual parsing validation ----------------
    let on_save_vil = {
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let keyboard_name = keyboard_name.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let lays = layers.clone();
            let names = layer_names.clone();
            let kb_name = (*keyboard_name).clone();
            let rows = *matrix_rows;
            let cols = *matrix_cols;
            let error = error.clone();
            
            spawn_local(async move {
                // Build the VIL JSON structure
                let layers_json: Vec<serde_json::Value> = lays
                    .iter()
                    .enumerate()
                    .map(|(idx, layer)| {
                        let name = names.get(idx).cloned().unwrap_or_else(|| format!("Layer{}", idx));
                        serde_json::json!({
                            "name": name,
                            "keycodes": layer.iter().map(|kc| format!("0x{:04X}", kc)).collect::<Vec<_>>()
                        })
                    })
                    .collect();
                
                let vil_data = serde_json::json!({
                    "version": 1,
                    "keyboard": kb_name,
                    "matrix": {
                        "rows": rows,
                        "cols": cols
                    },
                    "layers": layers_json
                });
                
                // Serialize to string
                let json_str = match serde_json::to_string_pretty(&vil_data) {
                    Ok(s) => s,
                    Err(e) => {
                        error.set(Some(format!("Failed to serialize keymap: {}", e)));
                        return;
                    }
                };
                
                // DUAL PARSING: Parse it back to verify it works
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(parsed) => {
                        // Verify the parsed data has the expected structure
                        if !parsed.get("layers").is_some() {
                            error.set(Some("Validation failed: missing layers in generated file".to_string()));
                            return;
                        }
                        
                        // Create and download the file
                        let blob = match web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(
                            &JsValue::from_str(&json_str),
                        )) {
                            Ok(b) => b,
                            Err(e) => {
                                error.set(Some(format!("Failed to create blob: {:?}", e)));
                                return;
                            }
                        };
                        
                        let url = match web_sys::Url::create_object_url_with_blob(&blob) {
                            Ok(u) => u,
                            Err(e) => {
                                error.set(Some(format!("Failed to create URL: {:?}", e)));
                                return;
                            }
                        };
                        
                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();
                        let link = document
                            .create_element("a")
                            .unwrap()
                            .dyn_into::<web_sys::HtmlAnchorElement>()
                            .unwrap();
                        
                        let filename = if kb_name.is_empty() {
                            "keymap.vil".to_string()
                        } else {
                            format!("{}_keymap.vil", kb_name.to_lowercase().replace(' ', "_"))
                        };
                        
                        link.set_href(&url);
                        link.set_download(&filename);
                        link.click();
                        
                        let _ = web_sys::Url::revoke_object_url(&url);
                        error.set(None); // Clear any previous errors
                    }
                    Err(e) => {
                        error.set(Some(format!("Dual parsing validation failed: {}", e)));
                    }
                }
            });
        })
    };

    // -- Download keymap as SVG -----------------------------------------------
    let on_download_svg = {
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let keyboard_name = keyboard_name.clone();
        let key_layout = key_layout.clone();
        let matrix_cols = matrix_cols.clone();
        let vial_protocol_ver = vial_protocol_ver.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let lays = layers.clone();
            let names = layer_names.clone();
            let kb_name = (*keyboard_name).clone();
            let layout = (*key_layout).clone();
            let cols = *matrix_cols;
            let protocol_ver = *vial_protocol_ver;
            let error = error.clone();
            
            spawn_local(async move {
                let svg = generate_vial_svg(&lays, &names, &kb_name, &layout, cols, protocol_ver);
                
                let blob = match web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(
                    &JsValue::from_str(&svg),
                )) {
                    Ok(b) => b,
                    Err(e) => {
                        error.set(Some(format!("Failed to create blob: {:?}", e)));
                        return;
                    }
                };
                
                let url = match web_sys::Url::create_object_url_with_blob(&blob) {
                    Ok(u) => u,
                    Err(e) => {
                        error.set(Some(format!("Failed to create URL: {:?}", e)));
                        return;
                    }
                };
                
                let window = web_sys::window().unwrap();
                let document = window.document().unwrap();
                let link = document
                    .create_element("a")
                    .unwrap()
                    .dyn_into::<web_sys::HtmlAnchorElement>()
                    .unwrap();
                
                let filename = if kb_name.is_empty() {
                    "keymap.svg".to_string()
                } else {
                    format!("{}_keymap.svg", kb_name.to_lowercase().replace(' ', "_"))
                };
                
                link.set_href(&url);
                link.set_download(&filename);
                link.click();
                
                let _ = web_sys::Url::revoke_object_url(&url);
                error.set(None);
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
                            <button onclick={on_save_vil.clone()} class="bg-blue-600 hover:bg-blue-700 text-white font-bold py-1.5 px-4 rounded shadow text-sm" title="Save keymap as .vil file">
                                {"Save as .vil"}
                            </button>
                            <button onclick={on_download_svg.clone()} class="bg-purple-600 hover:bg-purple-700 text-white font-bold py-1.5 px-4 rounded shadow text-sm" title="Download keymap as SVG">
                                {"Download SVG"}
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
                                &*keyboard_name,
                                &*layer_names,
                                &layer_menu_index,
                                &menu_focus_index,
                                &quick_assign_index,
                                &hint_map,
                                Rc::new(move |idx, up| move_layer(idx, up)),
                                Rc::new(move |idx| rename_layer(idx)),
                                Rc::new(move |idx| duplicate_layer(idx)),
                                Rc::new(move |idx| delete_layer(idx)),
                                Rc::new(move |idx| reset_layer(idx)),
                                Rc::new(move |idx| trans_to_none(idx)),
                                Rc::new(move |idx| none_to_trans(idx)),
                                Rc::new(move || start_quick_assign(())),
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
    layer_names: &[String],
    layer_menu_index: &UseStateHandle<Option<usize>>,
    menu_focus_index: &UseStateHandle<usize>,
    quick_assign_index: &UseStateHandle<Option<usize>>,
    hint_map: &std::collections::HashMap<String, HintTarget>,
    move_layer: Rc<dyn Fn(usize, bool)>,
    rename_layer: Rc<dyn Fn(usize)>,
    duplicate_layer: Rc<dyn Fn(usize)>,
    delete_layer: Rc<dyn Fn(usize)>,
    reset_layer: Rc<dyn Fn(usize)>,
    trans_to_none: Rc<dyn Fn(usize)>,
    none_to_trans: Rc<dyn Fn(usize)>,
    start_quick_assign: Rc<dyn Fn()>,
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

            let quick_assign_target = **quick_assign_index;

            html! {
                <div class="relative bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-xl p-8 overflow-auto shadow-inner w-full max-w-full"
                     style="min-height: 350px; height: 55vh;">
                    <div class="relative mx-auto" style={format!("width: {}px;", content_width_px)}>
                        { for key_layout.iter().enumerate().map(|(layout_idx, pos)| {
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
                            let is_quick_assign_target = quick_assign_target.map(|idx| idx == layout_idx).unwrap_or(false);
                            let key_class = if is_quick_assign_target {
                                "flex flex-col items-center justify-center font-bold border rounded-md bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 shadow-sm hover:shadow-md hover:border-blue-400 dark:hover:border-blue-500 transition-all cursor-pointer select-none relative ring-4 ring-blue-500 z-40"
                            } else {
                                "flex flex-col items-center justify-center font-bold border rounded-md bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 shadow-sm hover:shadow-md hover:border-blue-400 dark:hover:border-blue-500 transition-all cursor-pointer select-none relative"
                            };
                            html! {
                                <div
                                    onclick={onclick}
                                    class={key_class}
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
            let quick_assign_target = **quick_assign_index;
            html! {
                <div class="flex flex-wrap gap-1 my-4">
                    { for codes.iter().enumerate().map(|(keycode_idx, &kc)| {
                        let disp = keycode_display(kc, protocol_version, keyboard_name);
                        let hint = key_hints.get(keycode_idx);
                        let show_hint = jump_mode_active && hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                        let onclick = { let cb = on_key_click.clone(); Callback::from(move |_: MouseEvent| cb.emit(keycode_idx)) };
                        let is_quick_assign_target = quick_assign_target.map(|idx| idx == keycode_idx).unwrap_or(false);
                        let key_class = if is_quick_assign_target {
                            "w-14 h-14 flex flex-col items-center justify-center border rounded bg-gray-100 dark:bg-gray-800 border-gray-300 dark:border-gray-600 hover:bg-blue-100 dark:hover:bg-blue-900 cursor-pointer select-none relative ring-4 ring-blue-500 z-40"
                        } else {
                            "w-14 h-14 flex flex-col items-center justify-center border rounded bg-gray-100 dark:bg-gray-800 border-gray-300 dark:border-gray-600 hover:bg-blue-100 dark:hover:bg-blue-900 cursor-pointer select-none relative"
                        };
                        html! {
                            <div
                                onclick={onclick}
                                class={key_class}
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
                            let i_usize = i as usize;
                            let active_layer = active_layer.clone();
                            let layer_menu_index = layer_menu_index.clone();
                            let is_active = *active_layer == i;
                            let is_menu_open = *layer_menu_index == Some(i_usize);
                            let hint = layer_hints.get(i_usize);
                            let show_hint = jump_mode_active && hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                            let menu_trigger_hint = hint_map.iter().find(|(_, t)| **t == HintTarget::LayerMenu(i_usize)).map(|(h, _)| h);
                            let show_menu_trigger_hint = jump_mode_active && menu_trigger_hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                            let layer_name = layer_names.get(i_usize).map(|s| s.as_str()).unwrap_or("Layer");
                            html! {
                                <div class="relative" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                    <button
                                        class={classes!("px-4", "py-1.5", "rounded-md", "shadow-sm", "font-medium", "transition-all", "relative", "flex", "items-center", "gap-2",
                                            if is_active { "bg-white dark:bg-gray-700 text-blue-600 dark:text-blue-400" } else { "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300" }
                                        )}
                                        onclick={let cl = active_layer.clone(); Callback::from(move |_: MouseEvent| cl.set(i))}
                                    >
                                        {layer_name}
                                        <span
                                            onclick={let lmi = layer_menu_index.clone(); Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation();
                                                if *lmi == Some(i_usize) { lmi.set(None); } else { lmi.set(Some(i_usize)); }
                                            })}
                                            class="hover:bg-black/10 dark:hover:bg-white/10 rounded p-1 relative"
                                        >
                                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                                            { if show_menu_trigger_hint {
                                                let h = menu_trigger_hint.unwrap();
                                                let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                                html! { <div class="absolute -top-2 -right-2 bg-blue-400 dark:bg-blue-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                            } else { html! {} }}
                                        </span>
                                        { if show_hint {
                                            let h = hint.unwrap();
                                            let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                            html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                        } else { html! {} }}
                                    </button>
                                    { if is_menu_open {
                                        let move_up = move_layer.clone(); let move_dn = move_layer.clone(); let dup = duplicate_layer.clone(); let del = delete_layer.clone(); let ren = rename_layer.clone(); let res = reset_layer.clone();
                                        let batch_t_n = trans_to_none.clone(); let batch_n_t = none_to_trans.clone(); let qa = start_quick_assign.clone();
                                        let lmi = layer_menu_index.clone();
                                        let menu_items: Vec<(&str, Callback<MouseEvent>)> = vec![
                                            ("Move Up", Callback::from(move |_| move_up(i_usize, true))),
                                            ("Move Down", Callback::from(move |_| move_dn(i_usize, false))),
                                            ("Rename", Callback::from(move |_| ren(i_usize))),
                                            ("Duplicate", Callback::from(move |_| dup(i_usize))),
                                            ("Delete", Callback::from(move |_| del(i_usize))),
                                            ("Reset all to None", Callback::from(move |_| res(i_usize))),
                                            ("Trans → None", Callback::from(move |_| batch_t_n(i_usize))),
                                            ("None → Trans", Callback::from(move |_| batch_n_t(i_usize))),
                                            ("Quick Assignment", { let qa = qa.clone(); let lmi = lmi.clone(); Callback::from(move |_| { qa(); lmi.set(None); }) }),
                                        ];
                                        html! {
                                            <div class="absolute top-full left-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 z-50 py-1 overflow-hidden">
                                                { for menu_items.into_iter().enumerate().map(|(j, (label, cb))| {
                                                    let is_focused = **menu_focus_index == j;
                                                    let menu_hint = hint_map.iter().find(|(_, t)| **t == HintTarget::Menu(i_usize, j)).map(|(h, _)| h);
                                                    let show_menu_hint = jump_mode_active && menu_hint.map(|h| h.starts_with(jump_input)).unwrap_or(false);
                                                    let class = classes!("w-full", "text-left", "px-4", "py-2", "text-sm", "relative",
                                                        if is_focused { "bg-blue-100 dark:bg-blue-900/40" } else { "hover:bg-gray-100 dark:hover:bg-gray-700" },
                                                        if j == 4 { "text-red-500" } else if j == 5 { "text-orange-500" } else if j == 8 { "font-bold text-blue-500" } else { "" }
                                                    );
                                                    html! {
                                                        <>
                                                            { if j == 5 || j == 8 { html! { <div class="border-t border-gray-200 dark:border-gray-700 my-1"></div> } } else { html! {} } }
                                                            <button onclick={cb} class={class}>
                                                                {label}
                                                                { if show_menu_hint {
                                                                    let h = menu_hint.unwrap();
                                                                    let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                                                    html! { <div class="absolute top-0 right-0 bg-yellow-400 dark:bg-yellow-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-bl-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                                                } else { html! {} }}
                                                            </button>
                                                        </>
                                                    }
                                                })}
                                            </div>
                                        }
                                    } else { html! {} }}
                                </div>
                            }
                        })}
                    </div>
                }
            </div>

            { if let Some(idx) = **quick_assign_index {
                let current_key = idx + 1;
                let total_keys = key_layout.len();
                html! {
                    <div class="w-full mb-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-xl border border-blue-200 dark:border-blue-800">
                        <div class="flex justify-between items-center">
                            <div>
                                <h3 class="text-lg font-bold text-blue-800 dark:text-blue-300">{"Quick Assignment Mode"}</h3>
                                <p class="text-sm text-blue-600 dark:text-blue-400">{format!("Press keys on your keyboard to assign. Currently editing key {} of {}", current_key, total_keys)}</p>
                            </div>
                            <button onclick={let qa = quick_assign_index.clone(); Callback::from(move |_: MouseEvent| qa.set(None))} class="bg-blue-500 hover:bg-blue-600 text-white px-4 py-1 rounded-lg font-bold">{"Done"}</button>
                        </div>
                        <p class="text-xs text-blue-500 dark:text-blue-400 mt-2">{"Press Escape to exit, or click any key to jump to it."}</p>
                    </div>
                }
            } else { html! {} }}

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

/// Generate an SVG representation of the Vial keymap
fn generate_vial_svg(
    layers: &[Vec<u16>],
    layer_names: &[String],
    keyboard_name: &str,
    key_layout: &[MatrixPos],
    matrix_cols: u8,
    protocol_version: u32,
) -> String {
    if layers.is_empty() || key_layout.is_empty() {
        return String::new();
    }

    // Calculate layout bounds
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
    avg_w /= key_layout.len() as f32;

    let u_size = if avg_w < 5.0 { 1.0 } else if avg_w < 500.0 { 100.0 } else { 1000.0 };
    let size_scale = 44.0 / u_size;
    let u_pos = if max_x.abs() > 20000.0 || min_x.abs() > 20000.0 {
        19050.0
    } else {
        u_size
    };
    let pos_scale = 44.0 / u_pos;

    let layer_width = (max_x - min_x) * pos_scale;
    let layer_height = (max_y - min_y) * pos_scale;
    let padding = 60.0;

    let total_width = layer_width + 80.0;
    let total_height = (layer_height + padding) * layers.len() as f32 + 80.0;

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" style="background-color: #f8fafc;">"#,
        total_width, total_height, total_width, total_height
    );

    // Add styles
    svg.push_str(r#"<style>
        text { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
        .key { fill: white; stroke: #cbd5e1; stroke-width: 0.5; }
        .key:hover { fill: #f1f5f9; }
        .label { fill: #94a3b8; font-size: 5px; }
        .main-text { fill: #1e293b; font-size: 8px; font-weight: 600; }
        .layer-title { font-size: 16px; font-weight: bold; fill: #0f172a; }
        .keyboard-name { font-size: 12px; fill: #64748b; }
    </style>"#);

    // Add keyboard name at top
    if !keyboard_name.is_empty() {
        svg.push_str(&format!(
            r#"<text x="40" y="30" class="keyboard-name">{}</text>"#,
            escape_xml(keyboard_name)
        ));
    }

    // Render each layer
    for (l_idx, layer) in layers.iter().enumerate() {
        let y_offset = (layer_height + padding) * l_idx as f32 + 60.0;
        let name = layer_names.get(l_idx).cloned().unwrap_or_else(|| format!("L{}", l_idx));
        
        svg.push_str(&format!(
            r#"<text x="40" y="{}" class="layer-title">{}</text>"#,
            y_offset - 10.0,
            escape_xml(&name)
        ));

        let offset_x = -(min_x * pos_scale) + 40.0;
        let offset_y = -(min_y * pos_scale) + y_offset;

        for pos in key_layout.iter() {
            let keycode_idx = (pos.row as usize) * (matrix_cols as usize) + (pos.col as usize);
            let kc = layer.get(keycode_idx).copied().unwrap_or(0);
            let disp = keycode_display(kc, protocol_version, keyboard_name);

            let x = pos.x * pos_scale + offset_x;
            let y = pos.y * pos_scale + offset_y;
            let w = (pos.w * size_scale).max(20.0) - 4.0;
            let h = (pos.h * size_scale).max(20.0) - 4.0;

            svg.push_str(&format!(
                r#"<g transform="translate({}, {})">"#,
                x + w / 2.0, y + h / 2.0
            ));

            // Key rect
            svg.push_str(&format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" ry="3" class="key" />"#,
                -w / 2.0, -h / 2.0, w, h
            ));

            // Upper left label (function name like MO, TO, etc.)
            if !disp.upper_left.is_empty() {
                svg.push_str(&format!(
                    r#"<text x="{}" y="{}" class="label" text-anchor="start">{}</text>"#,
                    -w / 2.0 + 2.0,
                    -h / 2.0 + 5.0,
                    escape_xml(&disp.upper_left)
                ));
            }

            // Upper middle label
            if !disp.upper_middle.is_empty() {
                svg.push_str(&format!(
                    r#"<text x="0" y="{}" class="label" text-anchor="middle">{}</text>"#,
                    -h / 2.0 + 5.0,
                    escape_xml(&disp.upper_middle)
                ));
            }

            // Upper right label (for LT tap key, etc.)
            if !disp.upper_right.is_empty() {
                let display_tr = disp.upper_right.chars().take(6).collect::<String>();
                svg.push_str(&format!(
                    r#"<text x="{}" y="{}" class="label" text-anchor="end">{}</text>"#,
                    w / 2.0 - 2.0,
                    -h / 2.0 + 5.0,
                    escape_xml(&display_tr)
                ));
            }

            // Main label (center)
            let center_y = if !disp.upper_left.is_empty() || !disp.upper_right.is_empty() {
                1.0
            } else {
                0.0
            };
            let display_center = disp.middle.chars().take(8).collect::<String>();
            svg.push_str(&format!(
                r#"<text x="0" y="{}" class="main-text" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
                center_y,
                escape_xml(&display_center)
            ));

            svg.push_str("</g>");
        }
    }

    svg.push_str("</svg>");
    svg
}

fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}
