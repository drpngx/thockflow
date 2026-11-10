//! Keyboard layout manager — WebHID-based live configuration for Vial and
//! SVIL (Svalboard) keyboards. Both tabs share this UI; protocol differences
//! live in [`backend`].

use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

pub mod backend;
pub mod popup;
pub mod settings;
pub mod sval_settings;
pub mod svil_link;
pub mod webhid;
use popup::VialKeyBindingPopup;

pub use backend::Protocol;
use backend::{Link, SvilState};
use settings::QmkSettingsPanel;
use sval_settings::SvalSettingsPanel;
use vial_protocol::keycodes::keycode_display;
use vial_protocol::svil::file::SvilData;
use vial_protocol::vil::VilData;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum VialTab {
    Keymap,
    TestMatrix,
    QmkSettings,
    SvalSettings,
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
pub struct MatrixPos {
    pub row: u8,
    pub col: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// ---------------------------------------------------------------------------
// Page components
// ---------------------------------------------------------------------------

#[function_component]
pub fn VialHome() -> Html {
    html! { <KeyboardHome protocol={Protocol::Vial} /> }
}

#[function_component]
pub fn SvilHome() -> Html {
    html! { <KeyboardHome protocol={Protocol::Svil} /> }
}

/// Offer `text` as a file download.
fn download_text(filename: &str, text: &str) -> Result<(), String> {
    let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(text)))
        .map_err(|e| format!("Failed to create blob: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("Failed to create URL: {e:?}"))?;
    let document = web_sys::window().and_then(|w| w.document()).ok_or("No document")?;
    let link = document
        .create_element("a")
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "Not an anchor")?;
    link.set_href(&url);
    link.set_download(filename);
    link.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Let the user pick a file and read it as text.
async fn pick_text_file(accept: &str) -> Result<String, String> {
    use futures::channel::oneshot;
    let document = web_sys::window().and_then(|w| w.document()).ok_or("No document")?;
    let input = document
        .create_element("input")
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|_| "Not an input")?;
    input.set_type("file");
    input.set_accept(accept);
    let (tx, rx) = oneshot::channel::<Option<web_sys::File>>();
    let tx = std::cell::RefCell::new(Some(tx));
    let input_for_cb = input.clone();
    let on_change = Closure::<dyn FnMut()>::new(move || {
        let file = input_for_cb.files().and_then(|f| f.get(0));
        if let Some(tx) = tx.borrow_mut().take() {
            let _ = tx.send(file);
        }
    });
    input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    input.click();
    let file = rx.await.map_err(|_| "No file selected")?.ok_or("No file selected")?;
    drop(on_change);
    let text = wasm_bindgen_futures::JsFuture::from(file.text()).await.map_err(|e| format!("Failed to read file: {e:?}"))?;
    text.as_string().ok_or_else(|| "File is not text".to_string())
}

fn file_stem(kb_name: &str, fallback: &str) -> String {
    if kb_name.is_empty() {
        fallback.to_string()
    } else {
        kb_name.to_lowercase().replace(' ', "_")
    }
}

#[derive(Properties, PartialEq)]
pub struct KeyboardHomeProps {
    pub protocol: Protocol,
}

#[function_component]
pub fn KeyboardHome(props: &KeyboardHomeProps) -> Html {
    let protocol = props.protocol;
    let is_svil = protocol == Protocol::Svil;
    let connected = use_state(|| false);
    let device = use_state(|| Option::<Link>::None);
    let svil_state = use_state(|| Option::<SvilState>::None);
    let kb_uid = use_state(|| 0u64);
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
    let notice = use_state(|| Option::<String>::None);
    let loading = use_state(|| false);
    let busy = use_state(|| false);
    let matrix_state = use_state(Vec::<Vec<bool>>::new);
    let qmk_settings = use_state(Vec::<QmkSettingValue>::new);
    let definition_info = use_state(|| Option::<String>::None);
    let vial_protocol_ver = use_state(|| 0u32);
    let key_layout = use_state(Vec::<MatrixPos>::new);
    let jump_mode_active = use_state(|| false);
    let jump_input = use_state(|| String::new());
    let selected_key = use_state(|| None::<(usize, usize)>);
    let container_ref = use_node_ref();

    // Test Matrix tab states
    let displayed_layer = use_state(|| 0u8);

    // Layer menu state
    let layer_menu_index = use_state(|| None::<usize>);
    let menu_focus_index = use_state(|| 0usize);
    let quick_assign_index = use_state(|| None::<usize>);
    let layer_names = use_state(|| Vec::<String>::new());

    // Custom keycode labels are global to the keycode tables: drop them when
    // this page goes away.
    use_effect_with((), |_| || vial_protocol::keycodes::set_custom_keycodes(Vec::new()));

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

    // -- matrix polling effect (only active on TestMatrix tab) ------------
    // This effect ONLY runs when active_tab changes to TestMatrix
    {
        let device = device.clone();
        let matrix_state = matrix_state.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let active_tab_val = (*active_tab).clone();
        
        use_effect_with(active_tab_val, move |tab| {
            let mut cleanup: Option<Box<dyn FnOnce()>> = None;
            
            // Only start polling when on TestMatrix tab and device is connected
            if *tab == VialTab::TestMatrix && device.is_some() {
                // Clone device for the closure
                let dev_opt = (*device).clone();
                
                // Skip a tick while the previous poll is still in flight, so a
                // slow keyboard doesn't build up a queue of requests.
                let in_flight = Rc::new(std::cell::Cell::new(false));
                let closure = Closure::wrap(Box::new(move || {
                    if let Some(ref d) = dev_opt {
                        if in_flight.get() {
                            return;
                        }
                        in_flight.set(true);
                        let in_flight = in_flight.clone();
                        let matrix_state = matrix_state.clone();
                        let rows = *matrix_rows;
                        let cols = *matrix_cols;
                        let link = d.clone();

                        spawn_local(async move {
                            match link.poll_matrix(rows, cols).await {
                                Ok(new_state) => matrix_state.set(new_state),
                                Err(e) => log::warn!("Matrix poll error: {e}"),
                            }
                            in_flight.set(false);
                        });
                    }
                }) as Box<dyn FnMut()>);

                // Set up 50ms interval
                let window = web_sys::window().unwrap();
                let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    50,
                ).unwrap();

                cleanup = Some(Box::new(move || {
                    window.clear_interval_with_handle(interval_id);
                    drop(closure);
                }));
            }

            // Return cleanup function
            move || {
                if let Some(c) = cleanup {
                    c();
                }
            }
        });
    }

    // -- connect ------------------------------------------------------------
    // Puts a freshly loaded keyboard into the UI state.
    let apply_loaded = {
        let connected = connected.clone();
        let device = device.clone();
        let svil_state = svil_state.clone();
        let keyboard_name = keyboard_name.clone();
        let vendor_id = vendor_id.clone();
        let product_id = product_id.clone();
        let microcontroller = microcontroller.clone();
        let layer_count = layer_count.clone();
        let definition_info = definition_info.clone();
        let qmk_settings = qmk_settings.clone();
        let vial_protocol_ver = vial_protocol_ver.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let key_layout = key_layout.clone();
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let notice = notice.clone();
        let active_layer = active_layer.clone();
        let active_tab = active_tab.clone();
        let kb_uid = kb_uid.clone();
        Rc::new(move |link: Link, kb: backend::LoadedKeyboard| {
            kb_uid.set(kb.uid);
            keyboard_name.set(kb.name);
            vendor_id.set(kb.vendor_id);
            product_id.set(kb.product_id);
            microcontroller.set(kb.microcontroller);
            vial_protocol_ver.set(kb.protocol_version);
            matrix_rows.set(kb.rows);
            matrix_cols.set(kb.cols);
            layer_count.set(kb.layer_count);
            layers.set(kb.layers);
            layer_names.set(kb.layer_names);
            key_layout.set(kb.key_layout);
            qmk_settings.set(kb.qmk_settings);
            definition_info.set(kb.definition_info);
            notice.set(kb.notice);
            svil_state.set(kb.svil);
            active_layer.set(0);
            active_tab.set(VialTab::Keymap);
            device.set(Some(link));
            connected.set(true);
        })
    };

    let on_connect = {
        let error = error.clone();
        let loading = loading.clone();
        let apply_loaded = apply_loaded.clone();
        Callback::from(move |_: MouseEvent| {
            let error = error.clone();
            let loading = loading.clone();
            let apply_loaded = apply_loaded.clone();
            spawn_local(async move {
                loading.set(true);
                error.set(None);
                match backend::connect(protocol).await {
                    Ok((link, kb)) => apply_loaded(link, kb),
                    Err(e) => error.set(Some(e)),
                }
                loading.set(false);
            });
        })
    };

    // -- disconnect ---------------------------------------------------------
    let on_disconnect = {
        let connected = connected.clone();
        let device = device.clone();
        let svil_state = svil_state.clone();
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
        let displayed_layer = displayed_layer.clone();
        let matrix_state = matrix_state.clone();
        let notice = notice.clone();
        let active_tab = active_tab.clone();

        Callback::from(move |_: MouseEvent| {
            let device = device.clone();
            let connected = connected.clone();
            let svil_state = svil_state.clone();
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
            let displayed_layer = displayed_layer.clone();
            let matrix_state = matrix_state.clone();
            let notice = notice.clone();
            let active_tab = active_tab.clone();

            if let Some(link) = (*device).clone() {
                spawn_local(async move {
                    link.close().await;
                    vial_protocol::keycodes::set_custom_keycodes(Vec::new());
                    device.set(None);
                    svil_state.set(None);
                    connected.set(false);
                    keyboard_name.set(String::new());
                    vendor_id.set(0);
                    product_id.set(0);
                    microcontroller.set(String::new());
                    layers.set(Vec::new());
                    layer_count.set(0);
                    active_layer.set(0);
                    displayed_layer.set(0);
                    matrix_state.set(Vec::new());
                    qmk_settings.set(Vec::new());
                    layer_names.set(Vec::new());
                    layer_menu_index.set(None);
                    notice.set(None);
                    active_tab.set(VialTab::Keymap);
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
        let error = error.clone();
        move |idx: usize| {
            let window = web_sys::window().unwrap();
            let current_name = &layer_names[idx];
            if let Ok(Some(new_name)) =
                window.prompt_with_message_and_default("Rename layer:", current_name)
            {
                let trimmed = new_name.trim();
                // SVIL boards store names in 16 UTF-8 bytes.
                let valid = if is_svil {
                    vial_protocol::svil::validate_label(trimmed).map_err(|e| error.set(Some(e))).is_ok()
                } else {
                    trimmed.len() <= 32
                };
                if !trimmed.is_empty() && valid {
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
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let layer_count = layer_count.clone();
        let layer_names = layer_names.clone();
        let svil_state = svil_state.clone();
        let error = error.clone();
        let notice = notice.clone();
        let busy = busy.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(link) = (*device).clone() else { return };
            let lays = (*layers).clone();
            let names = (*layer_names).clone();
            let (rows, cols, count) = (*matrix_rows, *matrix_cols, *layer_count);
            let svil_state = svil_state.clone();
            let error = error.clone();
            let notice = notice.clone();
            let busy = busy.clone();
            spawn_local(async move {
                busy.set(true);
                let mut result = link.write_keymap(&lays, rows, cols, count).await;
                if result.is_ok() {
                    if let Some(st) = (*svil_state).clone() {
                        match link.write_layer_names(&names, &st.board_layer_names).await {
                            Ok(board) => svil_state.set(Some(SvilState { board_layer_names: board, ..st })),
                            Err(e) => result = Err(e),
                        }
                    }
                }
                match result {
                    Ok(()) => {
                        let extra = if lays.len() > count as usize {
                            format!(" Only the first {count} layers fit on the keyboard.")
                        } else {
                            String::new()
                        };
                        notice.set(Some(format!("Keymap saved to the keyboard.{extra}")));
                    }
                    Err(e) => error.set(Some(format!("Failed to save to the keyboard: {e}"))),
                }
                busy.set(false);
            });
        })
    };

    // -- Save as .vil (Vial keyboards): a full vial-gui layout file -----------
    let on_save_vil = {
        let device = device.clone();
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let keyboard_name = keyboard_name.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let layer_count = layer_count.clone();
        let key_layout = key_layout.clone();
        let qmk_settings = qmk_settings.clone();
        let vial_protocol_ver = vial_protocol_ver.clone();
        let kb_uid = kb_uid.clone();
        let error = error.clone();
        let busy = busy.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(link) = (*device).clone() else { return };
            let kb = backend::LoadedKeyboard {
                name: (*keyboard_name).clone(),
                uid: *kb_uid,
                protocol_version: *vial_protocol_ver,
                rows: *matrix_rows,
                cols: *matrix_cols,
                layer_count: *layer_count,
                layers: layers.iter().take(*layer_count as usize).cloned().collect(),
                layer_names: (*layer_names).clone(),
                key_layout: (*key_layout).clone(),
                qmk_settings: (*qmk_settings).clone(),
                ..Default::default()
            };
            let error = error.clone();
            let busy = busy.clone();
            spawn_local(async move {
                busy.set(true);
                match link.read_vil_backup(&kb).await {
                    Ok(data) => {
                        let text = data.to_json();
                        // Check the file reads back as written before offering it.
                        match VilData::from_json(&text) {
                            Ok(back) if back == data => {
                                let filename = format!("{}.vil", file_stem(&kb.name, "keyboard"));
                                match download_text(&filename, &text) {
                                    Ok(()) => error.set(None),
                                    Err(e) => error.set(Some(e)),
                                }
                            }
                            Ok(_) => error.set(Some("Validation failed: the .vil file doesn't read back as written.".into())),
                            Err(e) => error.set(Some(format!("Validation failed: {e}"))),
                        }
                    }
                    Err(e) => error.set(Some(format!("Failed to read the keyboard: {e}"))),
                }
                busy.set(false);
            });
        })
    };

    // -- Save as .svil (SVIL keyboards) ---------------------------------------
    let on_save_svil = {
        let device = device.clone();
        let svil_state = svil_state.clone();
        let keyboard_name = keyboard_name.clone();
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let qmk_settings = qmk_settings.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let layer_count = layer_count.clone();
        let error = error.clone();
        let busy = busy.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(link) = (*device).clone() else { return };
            let Some(st) = (*svil_state).clone() else { return };
            let kb = backend::LoadedKeyboard {
                name: (*keyboard_name).clone(),
                rows: *matrix_rows,
                cols: *matrix_cols,
                layer_count: *layer_count,
                // The file describes the keyboard: no more layers than it has.
                layers: layers.iter().take(*layer_count as usize).cloned().collect(),
                layer_names: (*layer_names).clone(),
                qmk_settings: (*qmk_settings).clone(),
                svil: Some(st.clone()),
                ..Default::default()
            };
            let error = error.clone();
            let busy = busy.clone();
            spawn_local(async move {
                busy.set(true);
                match link.read_svil_backup(&kb).await {
                    Ok(data) => {
                        let text = data.to_json(&st.keycode_names());
                        // Check the file reads back as written before offering it.
                        match SvilData::from_json(&text, &st.keycode_names()) {
                            Ok(back) if back == data => {
                                let filename = format!("{}.svil", file_stem(&kb.name, "keyboard"));
                                match download_text(&filename, &text) {
                                    Ok(()) => error.set(None),
                                    Err(e) => error.set(Some(e)),
                                }
                            }
                            Ok(_) => error.set(Some("Validation failed: the .svil file doesn't read back as written.".into())),
                            Err(e) => error.set(Some(format!("Validation failed: {e}"))),
                        }
                    }
                    Err(e) => error.set(Some(format!("Failed to read the keyboard: {e}"))),
                }
                busy.set(false);
            });
        })
    };

    // -- Open a .svil file into the editor (SVIL keyboards) -------------------
    let on_open_svil = {
        let device = device.clone();
        let svil_state = svil_state.clone();
        let layers = layers.clone();
        let layer_names = layer_names.clone();
        let qmk_settings = qmk_settings.clone();
        let matrix_rows = matrix_rows.clone();
        let matrix_cols = matrix_cols.clone();
        let layer_count = layer_count.clone();
        let error = error.clone();
        let notice = notice.clone();
        let busy = busy.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(link) = (*device).clone() else { return };
            let Some(st) = (*svil_state).clone() else { return };
            let current_layers = (*layers).clone();
            let current_names = (*layer_names).clone();
            let current_settings = (*qmk_settings).clone();
            let (rows, cols, count) = (*matrix_rows as usize, *matrix_cols as usize, *layer_count as usize);
            let svil_state = svil_state.clone();
            let layers = layers.clone();
            let layer_names = layer_names.clone();
            let qmk_settings = qmk_settings.clone();
            let error = error.clone();
            let notice = notice.clone();
            let busy = busy.clone();
            spawn_local(async move {
                let text = match pick_text_file(".svil,.vil,.viable,.json").await {
                    Ok(t) => t,
                    Err(e) => {
                        log::info!("No file opened: {e}");
                        return;
                    }
                };
                // A Vial .vil (e.g. saved from the Vial tab before flashing Svalboard QMK)
                // is migrated: Svalboard's USERnn keycodes become its SV_ keycodes.
                let mut migration_note = String::new();
                let parsed = if vial_protocol::vil::is_vil(&text) {
                    VilData::from_json(&text).map(|vil| {
                        let (data, report) = vil.to_svil(&st.keycode_names());
                        if report.unassigned_custom_keycodes > 0 {
                            migration_note.push_str(&format!(
                                " {} key(s) used custom keycodes Svalboard's Vial firmware left unassigned; they are now empty.",
                                report.unassigned_custom_keycodes
                            ));
                        }
                        if report.foreign_custom_keycodes > 0 {
                            migration_note.push_str(&format!(
                                " {} key(s) use another keyboard's custom keycodes (USERnn); they won't do anything here.",
                                report.foreign_custom_keycodes
                            ));
                        }
                        data
                    })
                } else {
                    SvilData::from_json(&text, &st.keycode_names())
                };
                let data = match parsed {
                    Ok(d) => d,
                    Err(e) => {
                        error.set(Some(format!("Couldn't open the file: {e}")));
                        return;
                    }
                };
                if data.uid != st.info.uid {
                    let ok = web_sys::window()
                        .and_then(|w| w.confirm_with_message("This layout was saved from a different keyboard. Open it anyway?").ok())
                        .unwrap_or(false);
                    if !ok {
                        return;
                    }
                }

                // Keymap and layer names go into the editor ("Save Keymap to Device" writes them).
                let file_rows = data.layout[0].len();
                let file_cols = data.layout[0][0].len();
                let mut new_layers = current_layers.clone();
                new_layers.resize(count.max(new_layers.len()), vec![0; rows * cols]);
                for (l, layer) in data.layout.iter().enumerate().take(count) {
                    for r in 0..rows.min(file_rows) {
                        for c in 0..cols.min(file_cols) {
                            if let Some(k) = new_layers[l].get_mut(r * cols + c) {
                                *k = layer[r][c];
                            }
                        }
                    }
                }
                let new_names: Vec<String> = (0..new_layers.len())
                    .map(|i| {
                        data.layer_names
                            .get(&i)
                            .cloned()
                            .unwrap_or_else(|| if i < count { backend::default_layer_name(i) } else { current_names.get(i).cloned().unwrap_or_default() })
                    })
                    .collect();
                layers.set(new_layers);
                layer_names.set(new_names);

                // Settings are applied to the keyboard right away, after asking.
                let settings_changes: Vec<(u16, Vec<u8>)> = current_settings
                    .iter()
                    .filter_map(|s| {
                        let v = *data.settings.get(&s.qsid)?;
                        let bytes = vial_protocol::svil::menus::int_to_bytes(v, s.value.len());
                        (bytes != s.value).then_some((s.qsid, bytes))
                    })
                    .collect();
                let value_changes: Vec<_> = data
                    .custom_values
                    .iter()
                    .filter_map(|cv| {
                        let current = st.menu_values.iter().find(|m| m.key == cv.key)?;
                        let mut bytes = cv.data.clone();
                        bytes.resize(current.data.len(), 0);
                        (bytes != current.data).then(|| (current.clone(), bytes))
                    })
                    .collect();
                let mut msg = format!(
                    "Opened the layout ({} layers). Review it, then use \"Save Keymap to Device\".",
                    data.layout.len().min(count)
                );
                if !settings_changes.is_empty() || !value_changes.is_empty() {
                    let ok = web_sys::window()
                        .and_then(|w| {
                            w.confirm_with_message(&format!(
                                "The file also has {} QMK setting(s) and {} keyboard setting(s) that differ from the keyboard. Apply them to the keyboard now?",
                                settings_changes.len(),
                                value_changes.len()
                            ))
                            .ok()
                        })
                        .unwrap_or(false);
                    if ok {
                        busy.set(true);
                        let mut settings = current_settings.clone();
                        let mut st = st.clone();
                        let mut failures = Vec::new();
                        for (qsid, bytes) in settings_changes {
                            match link.set_qmk_setting(qsid, &bytes).await {
                                Ok(()) => {
                                    if let Some(s) = settings.iter_mut().find(|s| s.qsid == qsid) {
                                        s.value = bytes;
                                    }
                                }
                                Err(e) => failures.push(format!("QMK setting {qsid}: {e}")),
                            }
                        }
                        for (entry, bytes) in value_changes {
                            match link.set_custom_value(entry.channel, entry.value_id, &bytes).await {
                                Ok(()) => {
                                    if let Some(m) = st.menu_values.iter_mut().find(|m| m.key == entry.key) {
                                        m.data = bytes;
                                    }
                                }
                                Err(e) => failures.push(format!("{}: {e}", entry.key)),
                            }
                        }
                        qmk_settings.set(settings);
                        svil_state.set(Some(st));
                        busy.set(false);
                        if !failures.is_empty() {
                            error.set(Some(format!("Some settings weren't applied: {}", failures.join("; "))));
                        }
                        msg.push_str(" Settings applied.");
                    }
                }
                let has_tables = !data.macros.iter().all(|m: &Vec<vial_protocol::svil::macros::MacroAction>| m.is_empty())
                    || !data.tap_dances.is_empty()
                    || !data.combos.is_empty()
                    || !data.key_overrides.is_empty()
                    || !data.alt_repeat_keys.is_empty()
                    || !data.leaders.is_empty()
                    || data.one_shot.is_some();
                if has_tables {
                    let ok = web_sys::window()
                        .and_then(|w| {
                            w.confirm_with_message(
                                "The file also has macros, tap dances, combos or key overrides. Write them to the keyboard now? \
                                 (Entries that differ from the keyboard are replaced.)",
                            )
                            .ok()
                        })
                        .unwrap_or(false);
                    if ok {
                        busy.set(true);
                        match link.write_svil_tables(&data, &st).await {
                            Ok(r) => {
                                msg.push_str(&format!(
                                    " Wrote {} table entr{}{}.",
                                    r.written,
                                    if r.written == 1 { "y" } else { "ies" },
                                    if r.macros_written { " and the macros" } else { "" }
                                ));
                                if r.dropped > 0 {
                                    msg.push_str(&format!(" {} entr(ies) didn't fit on the keyboard.", r.dropped));
                                }
                            }
                            Err(e) => error.set(Some(format!("Failed to write macros and tables: {e}"))),
                        }
                        busy.set(false);
                    }
                }
                msg.push_str(&migration_note);
                notice.set(Some(msg));
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
                if let Some(link) = (*device).as_ref() {
                    match link.set_qmk_setting(qsid, &value).await {
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
                if let Some(link) = (*device).as_ref() {
                    if let Err(e) = link.reset_qmk_settings().await {
                        error.set(Some(format!("Failed to reset: {e}")));
                    }
                }
            });
        })
    };

    // -- Keyboard (VIA3 menu) settings, SVIL only ---------------------------
    let on_sval_setting_change = {
        let device = device.clone();
        let error = error.clone();
        let svil_state = svil_state.clone();
        Callback::from(move |(r, data): (vial_protocol::svil::menus::ValueRef, Vec<u8>)| {
            let device = device.clone();
            let error = error.clone();
            let svil_state = svil_state.clone();
            spawn_local(async move {
                let Some(link) = (*device).clone() else { return };
                match link.set_custom_value(r.channel, r.value_id, &data).await {
                    Ok(()) => {
                        if let Some(mut st) = (*svil_state).clone() {
                            match st.menu_values.iter_mut().find(|e| e.key == r.key) {
                                Some(e) => e.data = data,
                                None => st.menu_values.push(vial_protocol::svil::menus::CustomValueEntry {
                                    key: r.key.clone(),
                                    channel: r.channel,
                                    value_id: r.value_id,
                                    data,
                                }),
                            }
                            svil_state.set(Some(st));
                        }
                    }
                    Err(e) => error.set(Some(format!("Failed to set {}: {e}", r.key))),
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
                            if *busy {
                                <span class="text-sm text-gray-500 italic">{"Working..."}</span>
                            }
                            <button onclick={on_save.clone()} disabled={*busy} class="ml-auto bg-green-600 hover:bg-green-700 disabled:opacity-50 text-white font-bold py-1.5 px-4 rounded shadow text-sm"
                                title={if is_svil { "Write the keymap and layer names to the keyboard" } else { "Write the keymap to the keyboard" }}>
                                {"Save Keymap to Device"}
                            </button>
                            if is_svil {
                                <button onclick={on_save_svil.clone()} disabled={*busy} class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white font-bold py-1.5 px-4 rounded shadow text-sm" title="Save a full .svil backup (keymap, layer names, settings, macros, tap dances, combos, ...)">
                                    {"Save as .svil"}
                                </button>
                                <button onclick={on_open_svil.clone()} disabled={*busy} class="bg-sky-600 hover:bg-sky-700 disabled:opacity-50 text-white font-bold py-1.5 px-4 rounded shadow text-sm" title="Open a .svil layout, or migrate a Vial .vil saved from the Vial tab">
                                    {"Open .svil / .vil"}
                                </button>
                            } else {
                                <button onclick={on_save_vil.clone()} disabled={*busy} class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white font-bold py-1.5 px-4 rounded shadow text-sm" title="Save a vial-gui .vil layout (keymap, macros, tap dances, combos, key overrides, QMK settings). Open it in the SVIL tab to migrate to Svalboard QMK.">
                                    {"Save as .vil"}
                                </button>
                            }
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

                    // Notice
                    if let Some(msg) = &*notice {
                        <div class="mb-4 p-3 bg-blue-50 dark:bg-blue-900/40 text-blue-800 dark:text-blue-200 rounded">
                            {msg}
                            <button
                                class="ml-2 underline"
                                onclick={let notice = notice.clone(); Callback::from(move |_: MouseEvent| notice.set(None))}
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
                            if svil_state.as_ref().is_some_and(|st| st.menus.is_some()) {
                                <button class={tab_class(&VialTab::SvalSettings)} onclick={set_tab(VialTab::SvalSettings)}>{"Sval Settings"}</button>
                            }
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
                            VialTab::TestMatrix => render_test_matrix_tab(
                                &matrix_state,
                                &key_layout,
                                &layers,
                                &displayed_layer,
                                &active_layer,
                                *matrix_cols,
                                *vial_protocol_ver,
                                &*keyboard_name,
                            ),
                            VialTab::QmkSettings => html! {
                                <QmkSettingsPanel
                                    settings={(*qmk_settings).clone()}
                                    on_change={on_setting_change}
                                    on_reset={on_settings_reset}
                                    loading={*loading}
                                />
                            },
                            VialTab::SvalSettings => match (*svil_state).as_ref() {
                                Some(st) => html! {
                                    <SvalSettingsPanel
                                        menus={st.menus.clone().unwrap_or_default()}
                                        values={st.menu_values.clone()}
                                        names={st.keycode_names()}
                                        on_change={on_sval_setting_change}
                                    />
                                },
                                None => html! {},
                            },
                        }}
                    } else {
                        <div class="text-center py-20 text-gray-500">
                            <p class="text-2xl mb-4">{if is_svil { "SVIL Keyboard Manager" } else { "Vial Keyboard Manager" }}</p>
                            <p>{if is_svil {
                                "Connect a Svalboard (or other SVIL keyboard running Svalboard QMK) via USB to get started."
                            } else {
                                "Connect a Vial-compatible keyboard via USB to get started."
                            }}</p>
                            <p class="text-sm mt-2">{"Requires a Chromium-based browser (Chrome, Edge) with WebHID support."}</p>
                        </div>
                    }
                </div>
            </div>
        </div>
    }
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

/// Determine which layer to display based on pressed keys
fn resolve_display_layer(
    matrix_state: &[Vec<bool>],
    layers: &[Vec<u16>],
    base_layer: u8,
    key_layout: &[MatrixPos],
    matrix_cols: u8,
    protocol_version: u32,
) -> u8 {
    for pos in key_layout.iter() {
        if pos.row as usize >= matrix_state.len() {
            continue;
        }
        if pos.col as usize >= matrix_state[pos.row as usize].len() {
            continue;
        }
        if matrix_state[pos.row as usize][pos.col as usize] {
            let keycode_idx = (pos.row as usize) * (matrix_cols as usize) + (pos.col as usize);
            let base_layer_idx = base_layer as usize;
            if base_layer_idx >= layers.len() {
                continue;
            }
            if keycode_idx >= layers[base_layer_idx].len() {
                continue;
            }
            let keycode = layers[base_layer_idx][keycode_idx];

            if let Some((func, target_layer)) = vial_protocol::keycodes::layer_info(keycode, protocol_version) {
                match func {
                    "TO" | "MO" | "DF" | "TG" | "OSL" | "TT" => return target_layer,
                    "LT" => return target_layer, // When held
                    _ => {}
                }
            }
        }
    }

    base_layer
}

fn render_test_matrix_tab(
    matrix_state: &UseStateHandle<Vec<Vec<bool>>>,
    key_layout: &UseStateHandle<Vec<MatrixPos>>,
    layers: &UseStateHandle<Vec<Vec<u16>>>,
    displayed_layer: &UseStateHandle<u8>,
    active_layer: &UseStateHandle<u8>,
    matrix_cols: u8,
    protocol_version: u32,
    keyboard_name: &str,
) -> Html {
    // Resolve which layer to display based on pressed keys
    let display_layer = resolve_display_layer(
        matrix_state,
        layers,
        **active_layer,
        key_layout,
        matrix_cols,
        protocol_version,
    );

    // Update displayed_layer state if it changed
    if display_layer != **displayed_layer {
        displayed_layer.set(display_layer);
    }

    let layer_keys = layers.get(display_layer as usize);

    // Show layer indicator if viewing a different layer than base
    let layer_indicator = if display_layer != **active_layer {
        html! {
            <div class="mb-4 px-4 py-2 bg-blue-100 dark:bg-blue-900/40 border border-blue-300 dark:border-blue-700 rounded-lg">
                <span class="text-sm font-medium text-blue-800 dark:text-blue-300">
                    {format!("Viewing Layer {} (Layer-switching key pressed)", display_layer)}
                </span>
            </div>
        }
    } else {
        html! {}
    };

    let layout_html = if let Some(codes) = layer_keys {
        if !key_layout.is_empty() {
            // Calculate layout bounds (same as render_keymap_tab)
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

                            // Check if this key is pressed
                            let row = pos.row as usize;
                            let col = pos.col as usize;
                            let is_pressed = row < matrix_state.len()
                                && col < matrix_state[row].len()
                                && matrix_state[row][col];

                            let x = (pos.x * pos_scale + offset_x) as i32;
                            let y = (pos.y * pos_scale) as i32;
                            let w = (pos.w * size_scale).max(20.0) as i32 - 4;
                            let h = (pos.h * size_scale).max(20.0) as i32 - 4;
                            let style = format!(
                                "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px;",
                                x, y, w, h
                            );

                            let key_class = if is_pressed {
                                "flex flex-col items-center justify-center font-bold border rounded-md bg-green-500 dark:bg-green-600 border-green-600 dark:border-green-700 shadow-lg shadow-green-500/50 dark:shadow-green-500/30 transition-all duration-75 select-none relative scale-95"
                            } else {
                                "flex flex-col items-center justify-center font-bold border rounded-md bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 shadow-sm hover:shadow-md hover:border-blue-400 dark:hover:border-blue-500 transition-all select-none relative"
                            };

                            html! {
                                <div
                                    class={key_class}
                                    style={style}
                                    title={format!("Row {}, Col {}
Keycode: 0x{:04X}", pos.row, pos.col, kc)}
                                >
                                    <div class="absolute top-0.5 left-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%]">{disp.upper_left}</div>
                                    <div class="absolute top-0.5 left-0 right-0 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-center mx-auto">{disp.upper_middle}</div>
                                    <div class="absolute top-0.5 right-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-right">{disp.upper_right}</div>
                                    <div class="text-[9px] leading-tight text-center px-1 break-all">{disp.middle}</div>
                                </div>
                            }
                        })}
                    </div>
                </div>
            }
        } else {
            // Fallback: simple grid with key labels
            html! {
                <div class="flex flex-wrap gap-1 my-4">
                    { for codes.iter().enumerate().map(|(keycode_idx, &kc)| {
                        let disp = keycode_display(kc, protocol_version, keyboard_name);

                        // Calculate row/col from index
                        let row = keycode_idx / (matrix_cols as usize);
                        let col = keycode_idx % (matrix_cols as usize);

                        // Check if this position is pressed
                        let is_pressed = if row < matrix_state.len() && col < matrix_state[row].len() {
                            matrix_state[row][col]
                        } else {
                            false
                        };

                        let key_class = if is_pressed {
                            "w-14 h-14 flex flex-col items-center justify-center border rounded bg-green-500 dark:bg-green-600 border-green-600 dark:border-green-700 shadow-lg transition-all duration-75 select-none relative scale-95"
                        } else {
                            "w-14 h-14 flex flex-col items-center justify-center border rounded bg-gray-100 dark:bg-gray-800 border-gray-300 dark:border-gray-600 hover:bg-gray-200 dark:hover:bg-gray-700 transition-all select-none relative"
                        };

                        html! {
                            <div
                                class={key_class}
                                title={format!("Row {}, Col {}
0x{:04X}", row, col, kc)}
                            >
                                <div class="absolute top-0.5 left-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%]">{disp.upper_left}</div>
                                <div class="absolute top-0.5 left-0 right-0 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-center mx-auto">{disp.upper_middle}</div>
                                <div class="absolute top-0.5 right-1 text-[7px] leading-tight opacity-50 truncate max-w-[30%] text-right">{disp.upper_right}</div>
                                <div class="text-[9px] leading-tight text-center px-1 break-all">{disp.middle}</div>
                            </div>
                        }
                    })}
                </div>
            }
        }
    } else {
        html! {
            <div class="py-12 text-center text-gray-400 italic">
                {"Loading matrix data..."}
            </div>
        }
    };

    html! {
        <div class="space-y-4">
            <div class="flex items-center justify-between">
                <div>
                    <h2 class="text-xl font-bold">{"Test Matrix"}</h2>
                    <p class="text-sm text-gray-500">
                        {"Press keys on your keyboard to test the switch matrix and see live layer switching."}
                    </p>
                </div>
                <div class="text-sm text-gray-500">
                    {format!("Base Layer: {}", **active_layer)}
                </div>
            </div>

            {layer_indicator}

            {layout_html}
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
