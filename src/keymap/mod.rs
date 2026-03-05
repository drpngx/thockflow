use yew::prelude::*;
use web_sys::{HtmlInputElement, FileReader};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;

pub mod behaviors;
use behaviors::{ZMK_BEHAVIORS, ParameterType};

pub mod layouts;

pub mod keycodes;
use keycodes::format_keycode;

use serde::{Serialize, Deserialize};

pub fn format_binding(binding: &str) -> String {
    let parts = get_binding_parts(binding);
    if parts.top_left.is_empty() && parts.top_right.is_empty() {
        parts.center
    } else if !parts.top_left.is_empty() && parts.top_right.is_empty() {
        format!("{}({})", parts.top_left, parts.center)
    } else {
        format!("{}({},{})", parts.top_left, parts.top_right, parts.center)
    }
}

pub struct BindingParts {
    pub top_left: String,
    pub top_right: String,
    pub center: String,
}

pub fn get_binding_parts(binding: &str) -> BindingParts {
    let parts: Vec<&str> = binding.split_whitespace().collect();
    if parts.is_empty() { return BindingParts { top_left: "".into(), top_right: "".into(), center: "".into() }; }
    
    let behavior_raw = parts[0];
    let behavior = behavior_raw.strip_prefix('&').unwrap_or(behavior_raw);
    let params = &parts[1..];

    match behavior_raw {
        "&kp" => BindingParts {
            top_left: "".into(),
            top_right: "".into(),
            center: params.get(0).map(|&p| format_keycode(p)).unwrap_or_else(|| "&kp".to_string()),
        },
        "&mo" | "&to" | "&tog" => BindingParts {
            top_left: behavior.into(),
            top_right: "".into(),
            center: params.get(0).cloned().unwrap_or("").to_string(),
        },
        "&sk" => BindingParts {
            top_left: "sk".into(),
            top_right: "".into(),
            center: params.get(0).map(|&p| format_keycode(p)).unwrap_or_else(|| "".to_string()),
        },
        "&lt" => BindingParts {
            top_left: "lt".into(),
            top_right: params.get(0).cloned().unwrap_or("").to_string(),
            center: params.get(1).map(|&p| format_keycode(p)).unwrap_or_else(|| "".to_string()),
        },
        "&mt" => {
            let mod_raw = params.get(0).map(|&s| s.strip_prefix("MOD_").unwrap_or(s)).unwrap_or("");
            let mod_short = if mod_raw.len() >= 2 { &mod_raw[..2] } else { mod_raw };
            BindingParts {
                top_left: "mt".into(),
                top_right: mod_short.to_string(),
                center: params.get(1).map(|&p| format_keycode(p)).unwrap_or_else(|| "".to_string()),
            }
        },
        "&trans" => BindingParts {
            top_left: "".into(),
            top_right: "".into(),
            center: "▽".into(),
        },
        "&none" => BindingParts {
            top_left: "".into(),
            top_right: "".into(),
            center: "".into(),
        },
        _ => {
            // Default: behavior in TL, first param in center if it exists
            BindingParts {
                top_left: behavior.into(),
                top_right: "".into(),
                center: params.get(0).cloned().unwrap_or("").to_string(),
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PhysicalKey {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub rotation: i32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub bindings: Vec<String>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct KeymapData {
    pub physical_layout: Vec<PhysicalKey>,
    pub layers: Vec<Layer>,
    pub includes: Vec<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SelectedKey {
    pub layer_index: usize,
    pub key_index: usize,
}

#[derive(Serialize)]
struct KeymapRequest {
    content: String,
}

#[function_component]
pub fn KeymapHome() -> Html {
    let keymap_data = use_state(|| None::<KeymapData>);
    let original_content = use_state(|| String::new());
    let error = use_state(|| None::<String>);
    let loading = use_state(|| false);

    let on_file_input = {
        let keymap_data = keymap_data.clone();
        let original_content = original_content.clone();
        let error = error.clone();
        let loading = loading.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    let reader = FileReader::new().unwrap();
                    let reader_c = reader.clone();
                    let keymap_data = keymap_data.clone();
                    let original_content = original_content.clone();
                    let error = error.clone();
                    let loading = loading.clone();

                    let onload = Closure::wrap(Box::new(move |_e: ProgressEvent| {
                        let content = reader_c.result().unwrap().as_string().unwrap();
                        original_content.set(content.clone());
                        log::info!("File read successfully, content length: {}", content.len());
                        let keymap_data = keymap_data.clone();
                        let error = error.clone();
                        let loading = loading.clone();
                        
                        loading.set(true);
                        spawn_local(async move {
                            log::info!("Sending parse request to server...");
                            let result = Request::post("/api/parse-keymap")
                                .json(&KeymapRequest { content })
                                .unwrap()
                                .send()
                                .await;

                            loading.set(false);
                            match result {
                                Ok(resp) => {
                                    log::info!("Server responded with status: {}", resp.status());
                                    if resp.ok() {
                                        match resp.json::<KeymapData>().await {
                                            Ok(data) => {
                                                log::info!("Successfully parsed keymap: {} keys, {} layers", data.physical_layout.len(), data.layers.len());
                                                keymap_data.set(Some(data));
                                                error.set(None);
                                            }
                                            Err(e) => {
                                                log::error!("Failed to decode JSON: {}", e);
                                                error.set(Some(format!("JSON Parse error: {}. (Check browser console for more details)", e)));
                                            }
                                        }
                                    } else {
                                        let err_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                                        log::error!("Server returned error: {}", err_text);
                                        error.set(Some(format!("Server error: {}", err_text)));
                                    }
                                }
                                Err(e) => {
                                    log::error!("Network error: {}", e);
                                    error.set(Some(format!("Failed to connect to server: {}", e)));
                                }
                            }
                        });
                    }) as Box<dyn FnMut(ProgressEvent)>);

                    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                    reader.read_as_text(&file).unwrap();
                    onload.forget();
                }
            }
        })
    };

    let on_update_data = {
        let keymap_data = keymap_data.clone();
        Callback::from(move |new_data: KeymapData| {
            keymap_data.set(Some(new_data));
        })
    };

    let on_save = {
        let keymap_data = keymap_data.clone();
        let original_content = original_content.clone();
        let error = error.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            if let Some(data) = &*keymap_data {
                let original_content_str = (*original_content).clone();
                let data = data.clone();
                let error = error.clone();
                let loading = loading.clone();
                
                loading.set(true);
                spawn_local(async move {
                    log::info!("Sending save request to server...");
                    let result = Request::post("/api/save-keymap")
                        .json(&SaveKeymapRequest { original_content: original_content_str, data })
                        .unwrap()
                        .send()
                        .await;

                    loading.set(false);
                    match result {
                        Ok(resp) => {
                            if resp.ok() {
                                let res = resp.json::<SaveKeymapResponse>().await.unwrap();
                                // Create a blob and download it
                                let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&res.content))).unwrap();
                                let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                                let window = web_sys::window().unwrap();
                                let document = window.document().unwrap();
                                let link = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                                link.set_href(&url);
                                link.set_download("edited.keymap");
                                link.click();
                                web_sys::Url::revoke_object_url(&url).unwrap();
                            } else {
                                error.set(Some(format!("Server error: {}", resp.text().await.unwrap_or_default())));
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Network error: {}", e)));
                        }
                    }
                });
            }
        })
    };

    html! {
        <div class="w-full flex flex-col items-center p-4">
            <h2 class="text-4xl font-display mb-8">{"ZMK Keymap Editor"}</h2>
            
            <div class="flex items-center space-x-4 mb-8">
                <div>
                    <label class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Upload .keymap file"}</label>
                    <input 
                        type="file" 
                        oninput={on_file_input}
                        class="block w-full text-sm text-gray-900 border border-gray-300 rounded-lg cursor-pointer bg-gray-50 dark:text-gray-400 focus:outline-none dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400"
                    />
                    <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">{"Type "} <kbd class="px-1.5 py-0.5 font-sans font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">{"j"}</kbd> {" to start jump mode"}</p>
                </div>
                { if keymap_data.is_some() {
                    html! {
                        <button 
                            onclick={on_save}
                            class="mt-6 px-6 py-2.5 bg-green-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-green-700 hover:shadow-lg focus:bg-green-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-green-800 active:shadow-lg transition duration-150 ease-in-out"
                        >
                            {"Save Keymap"}
                        </button>
                    }
                } else { html! {} }}
            </div>

            { if *loading {
                html! { <div class="text-blue-500 mb-4 animate-pulse">{"Processing..."}</div> }
            } else {
                html! {}
            }}

            { if let Some(err) = &*error {
                html! { <div class="text-red-500 mb-4">{err}</div> }
            } else {
                html! {}
            }}

            { if let Some(data) = &*keymap_data {
                html! { <KeymapRenderer data={data.clone()} on_update={on_update_data} /> }
            } else {
                if !*loading {
                    html! { <div class="text-gray-500 italic">{"Please upload a keymap file to start editing."}</div> }
                } else {
                    html! {}
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct RendererProps {
    pub data: KeymapData,
    pub on_update: Callback<KeymapData>,
}

#[function_component]
fn KeymapRenderer(props: &RendererProps) -> Html {
    let current_layer = use_state(|| 0);
    let selected_key = use_state(|| None::<SelectedKey>);
    let show_param_selection = use_state(|| false);
    
    // Jump mode state
    let jump_mode_active = use_state(|| false);
    let jump_input = use_state(|| String::new());
    let container_ref = use_node_ref();

    // Auto-focus on mount
    {
        let container_ref = container_ref.clone();
        use_effect(move || {
            if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
            || ()
        });
    }

    let layer = &props.data.layers[*current_layer];

    // Heuristic to determine scaling. 
    // Some ZMK physical layouts use centi-units (100 = 1u), 
    // others use micrometers (19050 = 1u) or 1000-units.
    let mut avg_w = 0.0;
    let mut max_x = 0;
    for pk in &props.data.physical_layout {
        avg_w += pk.width as f32;
        if pk.x.abs() > max_x { max_x = pk.x.abs(); }
    }
    if !props.data.physical_layout.is_empty() {
        avg_w /= props.data.physical_layout.len() as f32;
    }

    // Standard 1u size is usually 100 or 1000 in ZMK properties
    let u_size = if avg_w < 500.0 { 100.0 } else { 1000.0 };
    let size_scale = 44.0 / u_size;

    // If coordinates are very large, they are likely micrometers (1u = 19.05mm = 19050um)
    // Otherwise assume they use the same units as the key size.
    let u_pos = if max_x > 20000 { 19050.0 } else { u_size };
    let pos_scale = 44.0 / u_pos;

    let on_key_click = {
        let selected_key = selected_key.clone();
        let current_layer = current_layer.clone();
        Callback::from(move |key_index: usize| {
            selected_key.set(Some(SelectedKey {
                layer_index: *current_layer,
                key_index,
            }));
        })
    };

    // Hint generation logic: spatial-aware (row-by-row)
    // We sort indices by their physical position to assign "easier" hints to earlier keys
    let mut key_indices: Vec<usize> = (0..props.data.physical_layout.len()).collect();
    key_indices.sort_by(|&a, &b| {
        let ka = &props.data.physical_layout[a];
        let kb = &props.data.physical_layout[b];
        // Sort by row (y) then column (x)
        // Group by y in buckets of say 10 units to handle slight misalignments
        let ya = ka.y / 10;
        let yb = kb.y / 10;
        ya.cmp(&yb).then(ka.x.cmp(&kb.x))
    });

    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let mut hints = vec![String::new(); props.data.physical_layout.len()];
    for (i, &original_idx) in key_indices.iter().enumerate() {
        if i < hint_chars.len() * hint_chars.len() {
            let c1 = hint_chars.chars().nth(i / hint_chars.len()).unwrap();
            let c2 = hint_chars.chars().nth(i % hint_chars.len()).unwrap();
            hints[original_idx] = format!("{}{}", c1, c2);
        }
    }

    let on_keydown = {
        let jump_mode_active = jump_mode_active.clone();
        let jump_input = jump_input.clone();
        let selected_key = selected_key.clone();
        let on_key_click = on_key_click.clone();
        let hints_c = hints.clone();
        Callback::from(move |e: KeyboardEvent| {
            // If we are in a popup or input, don't handle jump keys here
            // But KeymapRenderer only has the container focused if no popup is open?
            // Actually, SelectedKey being Some means a popup is open.
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
                        
                        // Check if we matched a hint
                        if let Some(idx) = hints_c.iter().position(|h| h == &new_input) {
                            on_key_click.emit(idx);
                            jump_mode_active.set(false);
                            jump_input.set(String::new());
                        } else {
                            // Check if any hint still starts with this prefix
                            if hints_c.iter().any(|h| h.starts_with(&new_input)) {
                                jump_input.set(new_input);
                            } else {
                                // Invalid second char, reset or ignore?
                                // Ace-jump usually ignores or resets. Let's just not update if it doesn't match anything.
                            }
                        }
                        e.prevent_default();
                    }
                    _ => {}
                }
            } else {
                if e.key() == "j" {
                    jump_mode_active.set(true);
                    jump_input.set(String::new());
                    e.prevent_default();
                }
            }
        })
    };

    let close_popup = {
        let selected_key = selected_key.clone();
        let show_param_selection = show_param_selection.clone();
        let container_ref = container_ref.clone();
        Callback::from(move |_: MouseEvent| {
            selected_key.set(None);
            show_param_selection.set(false);
            // Re-focus container when popup closes
            if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
        })
    };

    let toggle_param_selection = {
        let show_param_selection = show_param_selection.clone();
        Callback::from(move |_: MouseEvent| {
            show_param_selection.set(!*show_param_selection);
        })
    };

    html! {
        <div 
            ref={container_ref}
            tabindex="0"
            onkeydown={on_keydown}
            class="flex flex-col items-center w-full mt-4 focus:outline-none"
        >
            <div class={classes!("flex", "flex-wrap", "justify-center", "gap-2", "mb-8", "bg-gray-100", "dark:bg-gray-800", "p-2", "rounded-2xl", "shadow-inner", "border", "border-gray-200", "dark:border-gray-700")}>
                { for props.data.layers.iter().enumerate().map(|(i, l)| {
                    let is_active = i == *current_layer;
                    let onclick = {
                        let current_layer = current_layer.clone();
                        Callback::from(move |_| current_layer.set(i))
                    };
                    let class = if is_active {
                        "px-6 py-2 bg-blue-500 text-white font-semibold rounded-xl shadow-md transition-all duration-200"
                    } else {
                        "px-6 py-2 bg-transparent text-gray-600 dark:text-gray-400 font-medium rounded-xl hover:bg-gray-200 dark:hover:bg-gray-700 transition-all duration-200 cursor-pointer"
                    };
                    html! {
                        <button onclick={onclick} class={classes!(class.split_whitespace().collect::<Vec<_>>())}>
                            {&l.name}
                        </button>
                    }
                })}
            </div>

            <div class={classes!("relative", "border", "dark:border-gray-600", "p-8", "rounded-xl", "bg-gray-50", "dark:bg-gray-800", "shadow-inner", "overflow-auto", "w-full", "max-w-5xl")} style="height: 600px;">
                { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| {
                    let binding = layer.bindings.get(i).cloned().unwrap_or_else(|| "".to_string());
                    let parts = get_binding_parts(&binding);
                    
                    let x = (pk.x as f32 * pos_scale) as i32 + 40;
                    let y = (pk.y as f32 * pos_scale) as i32 + 40;
                    
                    let w = (pk.width as f32 * size_scale).max(20.0) as i32 - 4;
                    let h = (pk.height as f32 * size_scale).max(20.0) as i32 - 4;

                    let rotation_deg = pk.rotation as f32 / 1000.0;
                    let style = format!("left: {}px; top: {}px; width: {}px; height: {}px; transform: rotate({}deg);", x, y, w, h, rotation_deg);
                    
                    let onclick = {
                        let on_key_click = on_key_click.clone();
                        Callback::from(move |_| on_key_click.emit(i))
                    };

                    let hint = hints.get(i);
                    let show_hint = *jump_mode_active && hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);

                    html! {
                        <div 
                            onclick={onclick}
                            class={classes!(
                                "absolute", "bg-white", "dark:bg-gray-700", "border", "border-gray-300", 
                                "dark:border-gray-500", "flex", "items-center", "justify-center", 
                                "font-mono", "rounded-md", "shadow-sm", 
                                "hover:border-blue-500", "cursor-pointer", "overflow-hidden", "text-center"
                            )}
                            style={style}
                            title={binding.clone()}
                        >
                            { if !parts.top_left.is_empty() {
                                html! { <span class="absolute top-0.5 left-0.5 text-[6px] text-gray-400 leading-none">{&parts.top_left}</span> }
                            } else { html! {} }}
                            
                            { if !parts.top_right.is_empty() {
                                html! { <span class="absolute top-0.5 right-0.5 text-[6px] text-gray-400 leading-none text-right max-w-[70%] truncate">{&parts.top_right}</span> }
                            } else { html! {} }}

                            <span class="truncate px-1 pointer-events-none text-[10px]">{&parts.center}</span>

                            { if show_hint {
                                let h = hint.unwrap();
                                let (prefix, suffix) = if jump_input.is_empty() {
                                    ("", h.as_str())
                                } else {
                                    (&h[..jump_input.len()], &h[jump_input.len()..])
                                };
                                html! {
                                    <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700">
                                        <span class="opacity-40">{prefix}</span>
                                        <span>{suffix}</span>
                                    </div>
                                }
                            } else { html! {} }}
                        </div>
                    }
                })}
            </div>

            { if let Some(sk) = &*selected_key {
                html! {
                    <KeyBindingPopup 
                        data={props.data.clone()} 
                        selected_key={sk.clone()} 
                        on_close={close_popup.clone()}
                        show_param_selection={*show_param_selection}
                        on_toggle_param_selection={toggle_param_selection.clone()}
                        on_update={props.on_update.clone()}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}


#[derive(Properties, PartialEq)]
pub struct PopupProps {
    pub data: KeymapData,
    pub selected_key: SelectedKey,
    pub on_close: Callback<MouseEvent>,
    pub show_param_selection: bool,
    pub on_toggle_param_selection: Callback<MouseEvent>,
    pub on_update: Callback<KeymapData>,
}

#[derive(Serialize)]
struct SaveKeymapRequest {
    original_content: String,
    data: KeymapData,
}

#[derive(Deserialize)]
struct SaveKeymapResponse {
    content: String,
}

#[derive(Clone, PartialEq, Debug)]
struct Suggestion {
    value: String,
    display: String,
}

#[function_component]
fn KeyBindingPopup(props: &PopupProps) -> Html {
    let filter = use_state(|| String::new());
    let suggestion_container_ref = use_node_ref();
    let input_ref = use_node_ref();
    let binding = &props.data.layers[props.selected_key.layer_index].bindings[props.selected_key.key_index];

    // Aggressive auto-focus input when a key is selected
    {
        let input_ref = input_ref.clone();
        use_effect_with(props.selected_key.clone(), move |_| {
            let timeout_cb = Closure::wrap(Box::new(move || {
                if let Some(input) = input_ref.cast::<web_sys::HtmlInputElement>() {
                    let _ = input.focus();
                    let _ = input.select();
                }
            }) as Box<dyn FnMut()>);
            
            let window = web_sys::window().expect("should have a window");
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_cb.as_ref().unchecked_ref(),
                50, // 50ms delay to ensure DOM is ready and settled
            );
            
            move || {
                drop(timeout_cb);
            }
        });
    }
    
    // Split binding into behavior and parameters
    let parts: Vec<&str> = binding.split_whitespace().collect();
    let initial_behavior_label = parts.get(0).cloned().unwrap_or("");
    let initial_params = parts[1..].iter().map(|&s| s.to_string()).collect::<Vec<String>>();

    let current_text = use_state(|| binding.clone());
    let current_behavior_label = use_state(|| initial_behavior_label.to_string());
    let current_params = use_state(|| initial_params.clone());
    let show_behavior_selection = use_state(|| false);
    
    // Autocomplete / Suggestions state
    let suggestion_index = use_state(|| 0usize);
    let show_suggestions = use_state(|| false);

    // Find behavior in metadata
    let behavior_name = current_behavior_label.strip_prefix('&').unwrap_or(&*current_behavior_label);
    let behavior_meta = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name);

    let display_name = behavior_meta.and_then(|m| m.display_name).unwrap_or(behavior_name);

    let selected_param_idx = use_state(|| 0usize);

    // Heuristic for "Modifier-only" parameter (for &sk and the first param of &mt)
    let is_modifier_only_param = |behavior_name: &str, param_idx: usize| {
        behavior_name == "sk" || (behavior_name == "mt" && param_idx == 0)
    };

    // Tricky behaviors that have variable parameter counts based on the first parameter
    let get_expected_param_count = |behavior_label: &str, params: &[String]| -> usize {
        let behavior_name = behavior_label.strip_prefix('&').unwrap_or(behavior_label);
        if let Some(meta) = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name) {
            if behavior_name == "bt" {
                if let Some(cmd) = params.get(0) {
                    if cmd == "BT_SEL" || cmd == "BT_DISC" {
                        return 2;
                    } else {
                        return 1;
                    }
                }
                return 1; // Default to 1 if no params yet
            }
            return meta.binding_cells as usize;
        }
        params.len() // Fallback
    };

    let expected_p_count = get_expected_param_count(&*current_behavior_label, &*current_params);

    let is_valid = {
        if let Some(meta) = behavior_meta {
            if current_params.len() != expected_p_count {
                false
            } else {
                let mut all_valid = true;
                for (i, p) in current_params.iter().enumerate() {
                    if p == "UNKNOWN" { 
                        all_valid = false;
                        break;
                    }
                    if let Some(ptype) = meta.parameter_metadata.get(i) {
                        match ptype {
                            ParameterType::Layer => {
                                // Must be a number or a valid layer name
                                if p.parse::<usize>().is_err() && !props.data.layers.iter().any(|l| &l.name == p) {
                                    all_valid = false;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                all_valid
            }
        } else {
            false // Unknown behavior is invalid
        }
    };

    let update_from_text = {
        let current_text = current_text.clone();
        let current_behavior_label = current_behavior_label.clone();
        let current_params = current_params.clone();
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |text: String| {
            current_text.set(text.clone());
            let parts: Vec<&str> = text.split_whitespace().collect();
            let new_behavior = parts.get(0).map(|&s| s.to_string()).unwrap_or_default();
            
            let params = parts[1..].iter().map(|&s| s.to_string()).collect::<Vec<String>>();
            
            current_behavior_label.set(new_behavior);
            current_params.set(params);
            show_suggestions.set(true);
        })
    };

    let on_apply = {
        let on_update = props.on_update.clone();
        let data = props.data.clone();
        let selected_key = props.selected_key.clone();
        let current_behavior_label = current_behavior_label.clone();
        let current_params = current_params.clone();
        let on_close = props.on_close.clone();
        let is_valid = is_valid;
        Callback::from(move |e: MouseEvent| {
            if !is_valid { return; }
            let mut new_data = data.clone();
            let mut new_binding = (*current_behavior_label).clone();
            for p in &*current_params {
                new_binding.push(' ');
                new_binding.push_str(p);
            }
            new_data.layers[selected_key.layer_index].bindings[selected_key.key_index] = new_binding;
            
            // Manage #includes
            let behavior_name = current_behavior_label.strip_prefix('&').unwrap_or(&*current_behavior_label);
            if let Some(meta) = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name) {
                if !meta.include_file.is_empty() && !new_data.includes.iter().any(|i| i == meta.include_file) {
                    new_data.includes.push(meta.include_file.to_string());
                }
            }
            
            on_update.emit(new_data);
            on_close.emit(e);
        })
    };

    let select_behavior = {
        let current_behavior_label = current_behavior_label.clone();
        let current_params = current_params.clone();
        let current_text = current_text.clone();
        let show_behavior_selection = show_behavior_selection.clone();
        Callback::from(move |b: &'static behaviors::ZmkBehavior| {
            let label = format!("&{}", b.label.unwrap_or(b.name));
            current_behavior_label.set(label.clone());
            
            // For behavior selection via UI, we force NEW parameters to be UNKNOWN
            let mut new_params = Vec::new();
            for _ in 0..b.binding_cells {
                new_params.push("UNKNOWN".to_string());
            }
            current_params.set(new_params.clone());
            
            let mut text = label;
            for p in &new_params {
                text.push(' ');
                text.push_str(p);
            }
            current_text.set(text);
            show_behavior_selection.set(false);
        })
    };

    let select_param_value = {
        let current_params = current_params.clone();
        let selected_param_idx = selected_param_idx.clone();
        let current_text = current_text.clone();
        let current_behavior_label = current_behavior_label.clone();
        let on_toggle_param_selection = props.on_toggle_param_selection.clone();
        let behavior_name_str = behavior_name.to_string();
        Callback::from(move |val: String| {
            let mut new_params = (*current_params).clone();
            if let Some(p) = new_params.get_mut(*selected_param_idx) {
                *p = val.clone();
            }
            
            // Special handling for dynamic parameter counts (like &bt)
            if behavior_name_str == "bt" && *selected_param_idx == 0 {
                if val == "BT_SEL" || val == "BT_DISC" {
                    if new_params.len() < 2 {
                        new_params.push("0".to_string());
                    }
                } else {
                    if new_params.len() > 1 {
                        new_params.truncate(1);
                    }
                }
            }
            
            current_params.set(new_params.clone());
            
            let mut text = (*current_behavior_label).clone();
            for p in &new_params {
                text.push(' ');
                text.push_str(p);
            }
            current_text.set(text);
            on_toggle_param_selection.emit(MouseEvent::new("click").unwrap());
        })
    };

    // Autocomplete logic
    let get_suggestions = {
        let text = (*current_text).clone();
        let behavior_meta = behavior_meta;
        let props_data = props.data.clone();
        move || -> Vec<Suggestion> {
            let parts: Vec<&str> = text.split_whitespace().collect();
            let has_trailing_space = text.ends_with(' ');
            
            let mut results: Vec<Suggestion> = if text.is_empty() || (text == "&" && !has_trailing_space) {
                ZMK_BEHAVIORS.iter().map(|b| {
                    let val = format!("&{}", b.label.unwrap_or(b.name));
                    let disp = if let Some(dn) = b.display_name { format!("{} ({})", val, dn) } else { val.clone() };
                    Suggestion { value: val, display: disp }
                }).collect()
            } else if !has_trailing_space && parts.len() == 1 {
                let query = parts[0].to_uppercase();
                ZMK_BEHAVIORS.iter()
                    .map(|b| {
                        let val = format!("&{}", b.label.unwrap_or(b.name));
                        let disp = if let Some(dn) = b.display_name { format!("{} ({})", val, dn) } else { val.clone() };
                        Suggestion { value: val, display: disp }
                    })
                    .filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query))
                    .collect()
            } else {
                let p_idx = if has_trailing_space { parts.len() - 1 } else { parts.len() - 2 };
                let query = if has_trailing_space { "" } else { parts.last().unwrap_or(&"") }.to_uppercase();
                
                if let Some(meta) = behavior_meta {
                    if let Some(p_type) = meta.parameter_metadata.get(p_idx) {
                        match p_type {
                            ParameterType::Layer => {
                                props_data.layers.iter().enumerate()
                                    .map(|(i, l)| Suggestion { value: i.to_string(), display: format!("{} ({})", i, l.name) })
                                    .filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query))
                                    .collect()
                            }
                            ParameterType::Modifier => {
                                keycodes::KEY_ALIASES.iter()
                                    .filter(|(&k, _)| keycodes::is_modifier(k))
                                    .map(|(&k, &v)| {
                                        let val = k.to_string();
                                        let disp = if k != v { format!("{} ({})", k, v) } else { val.clone() };
                                        Suggestion { value: val, display: disp }
                                    })
                                    .filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query))
                                    .collect()
                            }
                            ParameterType::Keycode => {
                                keycodes::KEY_ALIASES.iter()
                                    .map(|(&k, &v)| {
                                        let val = k.to_string();
                                        let disp = if k != v { format!("{} ({})", k, v) } else { val.clone() };
                                        Suggestion { value: val, display: disp }
                                    })
                                    .filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query))
                                    .collect()
                            }
                            ParameterType::Constant => {
                                let behavior_name = behavior_meta.and_then(|m| Some(m.label.unwrap_or(m.name))).unwrap_or("");
                                keycodes::KEY_ALIASES.iter()
                                    .filter(|(&k, _)| {
                                        match behavior_name {
                                            "mkp" => ["LCLK", "RCLK", "MCLK", "MB4", "MB5"].contains(&k),
                                            "mmv" => k.starts_with("MOVE_"),
                                            "msc" => k.starts_with("SCROLL_"),
                                            "bt" => k.starts_with("BT_"),
                                            "rgb_ug" => k.starts_with("RGB_"),
                                            "bl" => k.starts_with("BL_"),
                                            "out" => k.starts_with("OUT_"),
                                            "ext_power" => k.starts_with("EP_"),
                                            _ => k.starts_with("BT_") || k.starts_with("RGB_") || k.starts_with("OUT_") || 
                                                 k.starts_with("MOVE_") || k.starts_with("SCROLL_") || 
                                                 ["LCLK", "RCLK", "MCLK", "MB4", "MB5"].contains(&k)
                                        }
                                    })
                                    .map(|(&k, &v)| {
                                        let val = k.to_string();
                                        let disp = if k != v { format!("{} ({})", k, v) } else { val.clone() };
                                        Suggestion { value: val, display: disp }
                                    })
                                    .filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query))
                                    .collect()
                            }
                            _ => Vec::new()
                        }
                    } else { Vec::new() }
                } else { Vec::new() }
            };

            results.sort_by(|a, b| a.display.cmp(&b.display));
            results
        }
    };

    let suggestions = get_suggestions();

    let on_keydown = {
        let current_text = current_text.clone();
        let update_from_text = update_from_text.clone();
        let on_apply = on_apply.clone();
        let on_close = props.on_close.clone();
        let suggestions = suggestions.clone();
        let suggestion_index = suggestion_index.clone();
        let show_suggestions = show_suggestions.clone();
        let suggestion_container_ref = suggestion_container_ref.clone();
        Callback::from(move |e: KeyboardEvent| {
            match e.key().as_str() {
                "Enter" => {
                    e.prevent_default();
                    if *show_suggestions && !suggestions.is_empty() {
                        let selected = &suggestions[*suggestion_index].value;
                        let parts: Vec<&str> = (*current_text).split_whitespace().collect();
                        let has_trailing_space = (*current_text).ends_with(' ');
                        let mut new_text = String::new();
                        if has_trailing_space || parts.is_empty() {
                            new_text = format!("{}{} ", *current_text, selected);
                        } else {
                            for (i, p) in parts.iter().enumerate() {
                                if i == parts.len() - 1 {
                                    new_text.push_str(selected);
                                } else {
                                    new_text.push_str(p);
                                }
                                new_text.push(' ');
                            }
                        }
                        update_from_text.emit(new_text);
                        show_suggestions.set(false);
                    } else {
                        on_apply.emit(MouseEvent::new("click").unwrap());
                    }
                }
                "Escape" => {
                    on_close.emit(MouseEvent::new("click").unwrap());
                }
                "Tab" => {
                    if *show_suggestions && !suggestions.is_empty() {
                        e.prevent_default();
                        let selected = &suggestions[*suggestion_index].value;
                        let parts: Vec<&str> = (*current_text).split_whitespace().collect();
                        let has_trailing_space = (*current_text).ends_with(' ');
                        let mut new_text = String::new();
                        if has_trailing_space || parts.is_empty() {
                            new_text = format!("{}{} ", *current_text, selected);
                        } else {
                            for (i, p) in parts.iter().enumerate() {
                                if i == parts.len() - 1 {
                                    new_text.push_str(selected);
                                } else {
                                    new_text.push_str(p);
                                }
                                new_text.push(' ');
                            }
                        }
                        update_from_text.emit(new_text);
                        show_suggestions.set(false);
                    }
                }
                "ArrowDown" => {
                    if *show_suggestions && !suggestions.is_empty() {
                        e.prevent_default();
                        let next_idx = (*suggestion_index + 1) % suggestions.len();
                        suggestion_index.set(next_idx);
                        
                        // Scroll into view
                        if let Some(container) = suggestion_container_ref.cast::<web_sys::Element>() {
                            let items = container.get_elements_by_class_name("suggestion-item");
                            if let Some(item) = items.get_with_index(next_idx as u32) {
                                let item_el: web_sys::Element = item.dyn_into().unwrap();
                                item_el.scroll_into_view_with_bool(false); // Scroll to bottom if needed
                            }
                        }
                    }
                }
                "ArrowUp" => {
                    if *show_suggestions && !suggestions.is_empty() {
                        e.prevent_default();
                        let next_idx = if *suggestion_index == 0 {
                            suggestions.len() - 1
                        } else {
                            *suggestion_index - 1
                        };
                        suggestion_index.set(next_idx);
                        
                        // Scroll into view
                        if let Some(container) = suggestion_container_ref.cast::<web_sys::Element>() {
                            let items = container.get_elements_by_class_name("suggestion-item");
                            if let Some(item) = items.get_with_index(next_idx as u32) {
                                let item_el: web_sys::Element = item.dyn_into().unwrap();
                                item_el.scroll_into_view_with_bool(true); // Scroll to top if needed
                            }
                        }
                    }
                }
                _ => {
                    show_suggestions.set(true);
                    suggestion_index.set(0);
                }
            }
        })
    };

    // Mini-map scaling
    let mut max_x = 0;
    for pk in &props.data.physical_layout {
        if pk.x.abs() > max_x { max_x = pk.x.abs(); }
    }
    let u_pos = if max_x > 20000 { 19050.0 } else if max_x > 500 { 1000.0 } else { 100.0 };
    let mini_scale = 10.0 / u_pos;

    // Use current states for preview
    let mut current_binding_full = (*current_behavior_label).clone();
    for p in &*current_params {
        current_binding_full.push(' ');
        current_binding_full.push_str(p);
    }
    let preview_parts = get_binding_parts(&current_binding_full);
    
    let selected_pk = &props.data.physical_layout[props.selected_key.key_index];
    let rotation_deg = selected_pk.rotation as f32 / 1000.0;
    let preview_style = format!("transform: rotate({}deg);", rotation_deg);

    let tl = if !preview_parts.top_left.is_empty() {
        html! { <span class="absolute top-1 left-1 text-[8px] text-gray-400 leading-none">{preview_parts.top_left}</span> }
    } else {
        html! {}
    };
    let tr = if !preview_parts.top_right.is_empty() {
        html! { <span class="absolute top-1 right-1 text-[8px] text-gray-400 leading-none text-right max-w-[70%] truncate">{preview_parts.top_right}</span> }
    } else {
        html! {}
    };

    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
            <div class="bg-[#1a202c] text-white rounded-lg shadow-2xl flex max-w-5xl w-full overflow-hidden border border-gray-700 h-[80vh]">
                <div class="flex-1 p-8 overflow-y-auto flex flex-col">
                    // Keyboard mini-map
                    <div class="flex justify-center mb-8 relative h-32 w-full shrink-0">
                        <div class="relative">
                            { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| {
                                let is_selected = i == props.selected_key.key_index;
                                let x = (pk.x as f32 * mini_scale) as i32;
                                let y = (pk.y as f32 * mini_scale) as i32;
                                let w = (pk.width as f32 * mini_scale).max(4.0) as i32 - 1;
                                let h = (pk.height as f32 * mini_scale).max(4.0) as i32 - 1;
                                let rotation_deg = pk.rotation as f32 / 1000.0;
                                let style = format!("left: {}px; top: {}px; width: {}px; height: {}px; transform: rotate({}deg);", x, y, w, h, rotation_deg);
                                let class = if is_selected { "bg-green-500" } else { "bg-gray-700" };
                                html! { <div class={classes!("absolute", "rounded-sm", class)} style={style} /> }
                            })}
                        </div>
                        
                        // Arrow and current binding preview
                        <div class="flex items-center ml-24 space-x-8">
                            <span class="text-2xl text-gray-400">{"→"}</span>
                            <div class="bg-gray-800 w-16 h-16 rounded-lg border border-gray-600 flex items-center justify-center relative font-mono shadow-inner" style={preview_style}>
                                {tl}
                                {tr}
                                <span class="text-xl font-bold">{preview_parts.center}</span>
                            </div>
                        </div>
                    </div>

                    // Text Input
                    <div class="mb-6 shrink-0">
                        <input 
                            ref={input_ref}
                            type="text" 
                            class={classes!(
                                "w-full", "bg-gray-900", "border", "text-2xl", "p-4", "rounded", "font-mono", "focus:outline-none",
                                if is_valid { vec!["border-gray-600", "focus:border-blue-500"] } else { vec!["border-red-500", "focus:border-red-400", "text-red-200"] }
                            )}
                            value={(*current_text).clone()}
                            oninput={let update = update_from_text.clone(); Callback::from(move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                update.emit(input.value());
                            })}
                            onkeydown={on_keydown}
                        />
                        { if !is_valid {
                            html! { <div class="text-red-400 text-sm mt-1">{"Invalid binding: incomplete or incorrect parameters."}</div> }
                        } else { html! {} }}
                    </div>

                    <div class="border-t border-gray-700 my-6 shrink-0"></div>

                    // Behavior Selection
                    <div class="mb-6 shrink-0">
                        <div class="flex items-center space-x-4">
                            <span class="text-xl font-semibold w-24">{"Behavior"}</span>
                            <div 
                                onclick={let show = show_behavior_selection.clone(); Callback::from(move |_| show.set(!*show))}
                                class="flex items-center space-x-2 bg-gray-800 px-3 py-1 rounded border border-gray-600 border-dashed cursor-pointer hover:bg-gray-700"
                            >
                                <span class="text-gray-300 font-mono">{&*current_behavior_label}</span>
                                <span class="text-gray-500">{"|"}</span>
                                <span class="text-gray-300">{display_name}</span>
                            </div>
                        </div>
                        
                        { if *show_behavior_selection {
                            html! {
                                <div class="mt-4 grid grid-cols-3 gap-2 max-h-48 overflow-y-auto bg-black p-2 rounded border border-gray-700">
                                    { for ZMK_BEHAVIORS.iter().map(|b| {
                                        let b_c = b;
                                        let onclick = {
                                            let select = select_behavior.clone();
                                            Callback::from(move |_| select.emit(b_c))
                                        };
                                        html! {
                                            <div onclick={onclick} class="p-2 text-xs hover:bg-gray-800 cursor-pointer rounded border border-gray-800">
                                                <div class="font-mono text-blue-400">{"&"}{b.label.unwrap_or(b.name)}</div>
                                                <div class="text-gray-500 truncate">{b.display_name.unwrap_or("")}</div>
                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        } else { html! {} }}
                    </div>

                    // Parameters
                    <div class="mb-10 grow overflow-y-auto">
                        <div class="text-xl font-semibold mb-4">{"Parameters"}</div>
                        { if let Some(meta) = behavior_meta {
                            html! {
                                <div class="flex flex-col space-y-4 ml-8">
                                { for (0..expected_p_count).map(|i| {
                                    let ptype = meta.parameter_metadata.get(i).cloned().unwrap_or(ParameterType::Constant);
                                    let value = current_params.get(i).cloned().unwrap_or("UNKNOWN".to_string());
                                    let label = match ptype {
                                        ParameterType::Layer => "Layer",
                                        ParameterType::Keycode => "Keycode",
                                        ParameterType::Modifier => "Modifier",
                                        ParameterType::Constant => "Constant",
                                        ParameterType::None => "None",
                                    };
                                    
                                    let display_value = match ptype {
                                        ParameterType::Layer => {
                                            if let Ok(idx) = value.parse::<usize>() {
                                                props.data.layers.get(idx).map(|l| l.name.as_str()).unwrap_or(&value)
                                            } else {
                                                // Try to find by name
                                                props.data.layers.iter().find(|l| l.name == value).map(|l| l.name.as_str()).unwrap_or(&value)
                                            }.to_string()
                                        }
                                        ParameterType::Keycode | ParameterType::Constant | ParameterType::Modifier => format_keycode(&value),
                                        _ => value.to_string(),
                                    };

                                    let is_active = props.show_param_selection && *selected_param_idx == i;

                                    let onclick = {
                                        let on_toggle_param_selection = props.on_toggle_param_selection.clone();
                                        let selected_param_idx = selected_param_idx.clone();
                                        let show_param_selection = props.show_param_selection;
                                        Callback::from(move |e: MouseEvent| {
                                            if !show_param_selection || *selected_param_idx != i {
                                                selected_param_idx.set(i);
                                                if !show_param_selection {
                                                    on_toggle_param_selection.emit(e);
                                                }
                                            } else {
                                                on_toggle_param_selection.emit(e);
                                            }
                                        })
                                    };

                                    html! {
                                        <div class="flex items-center space-x-4">
                                            <span class="text-gray-400 w-16">{label}</span>
                                            <div 
                                                onclick={onclick}
                                                class={classes!(
                                                    "flex", "items-center", "space-x-2", "px-3", "py-1", "rounded", "border", "cursor-pointer",
                                                    if is_active { vec!["bg-green-600", "border-green-400"] } else if value == "UNKNOWN" { vec!["bg-red-900", "border-red-500", "border-dashed"] } else { vec!["bg-gray-800", "border-gray-600", "border-dashed"] }
                                                )}
                                            >
                                                <span class="font-mono">{value}</span>
                                                <span class="text-gray-500">{"|"}</span>
                                                <span class="">{display_value}</span>
                                            </div>
                                        </div>
                                    }
                                })}
                                </div>
                            }
                        } else {
                            html! {
                                <div class="ml-8 text-gray-500 italic">{"No metadata for this behavior."}</div>
                            }
                        }}
                    </div>

                    // Actions
                    <div class="flex justify-between space-x-4 mt-auto shrink-0 pt-4">
                        <button onclick={props.on_close.clone()} class="bg-gray-700 hover:bg-gray-600 text-white px-8 py-2 rounded font-semibold transition-colors">
                            {"Cancel"}
                        </button>
                        <button 
                            disabled={!is_valid}
                            onclick={on_apply} 
                            class={classes!(
                                "px-8", "py-2", "rounded", "font-semibold", "transition-colors",
                                if is_valid { vec!["bg-green-600", "hover:bg-green-700", "text-white"] } else { vec!["bg-gray-800", "text-gray-500", "cursor-not-allowed"] }
                            )}
                        >
                            {"Apply"}
                        </button>
                    </div>
                </div>

                // Side Panel for Autocomplete / Parameter Selection
                <div class="w-80 bg-black border-l border-gray-700 flex flex-col h-full">
                    { if *show_suggestions && !suggestions.is_empty() {
                        let text_val = (*current_text).clone();
                        let update = update_from_text.clone();
                        let show_sug = show_suggestions.clone();
                        html! {
                            <div class="flex-1 flex flex-col overflow-hidden">
                                <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest shrink-0">{"Suggestions"}</div>
                                <div class="flex-1 overflow-y-auto" ref={suggestion_container_ref}>
                                    { for suggestions.iter().enumerate().map(|(i, s)| {
                                        let is_active = i == *suggestion_index;
                                        let val = s.value.clone();
                                        let text_val = text_val.clone();
                                        let update = update.clone();
                                        let show_sug = show_sug.clone();
                                        let onclick = Callback::from(move |_| {
                                            let parts: Vec<&str> = text_val.split_whitespace().collect();
                                            let has_trailing_space = text_val.ends_with(' ');
                                            let mut new_text = String::new();
                                            if has_trailing_space || parts.is_empty() {
                                                new_text = format!("{}{} ", text_val, val);
                                            } else {
                                                for (j, p) in parts.iter().enumerate() {
                                                    if j == parts.len() - 1 {
                                                        new_text.push_str(&val);
                                                    } else {
                                                        new_text.push_str(p);
                                                    }
                                                    new_text.push(' ');
                                                }
                                            }
                                            update.emit(new_text);
                                            show_sug.set(false);
                                        });
                                        html! {
                                            <div 
                                                onclick={onclick}
                                                class={classes!(
                                                    "suggestion-item", "p-3", "border-b", "border-gray-900", "cursor-pointer", "hover:bg-gray-900", "transition-colors", "font-mono", "text-sm",
                                                    if is_active { vec!["bg-blue-900", "text-white", "border-blue-700"] } else { vec!["text-gray-300"] }
                                                )}
                                            >
                                                {&s.display}
                                            </div>
                                        }
                                    })}
                                </div>
                            </div>
                        }
                    } else if props.show_param_selection {
                        let p_idx = *selected_param_idx;
                        let p_type = behavior_meta.and_then(|m| m.parameter_metadata.get(p_idx)).cloned().unwrap_or(ParameterType::None);
                        
                        match p_type {
                            ParameterType::Layer => html! {
                                <div class="flex-1 flex flex-col h-full">
                                    <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest">{"Select Layer"}</div>
                                    <div class="flex-1 overflow-y-auto">
                                        { for props.data.layers.iter().enumerate().map(|(i, l)| {
                                            let is_active = current_params.get(p_idx).map(|p| *p == i.to_string()).unwrap_or(false);
                                            let val = i.to_string();
                                            let select = select_param_value.clone();
                                            let onclick = Callback::from(move |_| select.emit(val.clone()));
                                            html! {
                                                <div onclick={onclick} class={classes!(
                                                    "p-4", "border-b", "border-gray-800", "cursor-pointer", "hover:bg-gray-900", "transition-colors",
                                                    if is_active { "bg-white text-black" } else { "" }
                                                )}>
                                                    <div class="font-bold">{i}</div>
                                                    <div class={if is_active { "text-gray-600 italic" } else { "text-gray-400 italic" }}>{&l.name}</div>
                                                </div>
                                            }
                                        })}
                                    </div>
                                    <div class="p-2 flex justify-center border-t border-gray-700">
                                        <button onclick={props.on_toggle_param_selection.clone()} class="text-xs text-gray-400 hover:text-white uppercase tracking-widest py-1 flex items-center">
                                            <span class="rotate-90 inline-block mr-1">{"Close"}</span>
                                        </button>
                                    </div>
                                </div>
                            },
                            ParameterType::Keycode => {
                                let filter_val = (*filter).clone();
                                let behavior_name = behavior_meta.and_then(|m| Some(m.label.unwrap_or(m.name))).unwrap_or("");
                                let only_mods = is_modifier_only_param(behavior_name, p_idx);
                                
                                html! {
                                    <div class="flex-1 flex flex-col h-full">
                                        <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest">
                                            {if only_mods { "Select Modifier" } else { "Select Keycode" }}
                                        </div>
                                        <div class="p-2 border-b border-gray-700">
                                            <input 
                                                type="text" 
                                                placeholder="Search..." 
                                                class="w-full bg-gray-900 text-white text-xs p-1 rounded focus:outline-none focus:ring-1 focus:ring-blue-500" 
                                                oninput={let filter = filter.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    filter.set(input.value().to_uppercase());
                                                })}
                                                value={filter_val.clone()}
                                            />
                                        </div>
                                        <div class="flex-1 overflow-y-auto">
                                            { for keycodes::KEY_ALIASES.iter()
                                                .filter(|(&k, _)| !only_mods || keycodes::is_modifier(k))
                                                .filter(|(&k, &v)| k.to_uppercase().contains(&filter_val) || v.to_uppercase().contains(&filter_val))
                                                .map(|(&k, &v)| {
                                                let val = k.to_string();
                                                let select = select_param_value.clone();
                                                let is_active = current_params.get(p_idx).map(|p| *p == val).unwrap_or(false);
                                                let val_c = val.clone();
                                                let onclick = Callback::from(move |_| select.emit(val_c.clone()));
                                                html! {
                                                    <div onclick={onclick} class={classes!(
                                                        "p-2", "border-b", "border-gray-800", "cursor-pointer", "hover:bg-gray-900", "transition-colors", "text-xs",
                                                        if is_active { "bg-white text-black" } else { "" }
                                                    )}>
                                                        <div class="font-bold font-mono">{val}</div>
                                                        <div class={if is_active { "text-gray-600" } else { "text-gray-400" }}>{v}</div>
                                                    </div>
                                                }
                                            })}
                                        </div>
                                        <div class="p-2 flex justify-center border-t border-gray-700">
                                            <button onclick={props.on_toggle_param_selection.clone()} class="text-xs text-gray-400 hover:text-white uppercase tracking-widest py-1 flex items-center">
                                                <span class="rotate-90 inline-block mr-1">{"Close"}</span>
                                            </button>
                                        </div>
                                    </div>
                                }
                            },
                            _ => html! {
                                <div class="flex-1 flex flex-col items-center justify-center p-4 text-center">
                                    <div class="text-gray-500 italic">{"Selection not implemented for this parameter type."}</div>
                                    <button onclick={props.on_toggle_param_selection.clone()} class="mt-4 text-xs text-gray-400 hover:text-white uppercase tracking-widest">
                                        {"Close"}
                                    </button>
                                </div>
                            }
                        }
                    } else {
                        html! {
                            <div class="flex-1 flex flex-col items-center justify-center p-8 text-center text-gray-600">
                                <div class="text-4xl mb-4">{"⌨️"}</div>
                                <div class="text-sm">{"Type to see suggestions or click a parameter to select from list."}</div>
                            </div>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
