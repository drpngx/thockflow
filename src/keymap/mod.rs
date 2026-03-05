use yew::prelude::*;
use web_sys::HtmlInputElement;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = showOpenFilePicker)]
    fn show_open_file_picker(options: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = getFile)]
    fn get_file(this: &FileSystemFileHandle) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = createWritable)]
    fn create_writable(this: &FileSystemFileHandle) -> js_sys::Promise;

    #[derive(Clone, PartialEq)]
    #[wasm_bindgen]
    pub type FileSystemFileHandle;

    #[wasm_bindgen(method, getter)]
    fn name(this: &FileSystemFileHandle) -> String;

    #[wasm_bindgen(method, js_name = write)]
    fn write(this: &FileSystemWritableFileStream, data: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = close)]
    fn close(this: &FileSystemWritableFileStream) -> js_sys::Promise;

    #[derive(Clone, PartialEq)]
    #[wasm_bindgen]
    pub type FileSystemWritableFileStream;
}

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
        "&kp" => {
            let p = params.get(0).cloned().unwrap_or("");
            BindingParts {
                top_left: "".into(),
                top_right: keycodes::get_keycode_shifted(p).map(|s| s.to_string()).unwrap_or_default(),
                center: format_keycode(p),
            }
        },
        "&gresc" => BindingParts {
            top_left: "".into(),
            top_right: "~ `".into(),
            center: "Esc".into(),
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
        "&bt" => {
            let cmd_raw = params.get(0).unwrap_or(&"");
            let cmd = cmd_raw.strip_prefix("BT_").unwrap_or(cmd_raw);
            let val = params.get(1).map(|&s| s.to_string()).unwrap_or_default();

            let is_single_param = ["BT_CLR", "BT_NXT", "BT_NEXT", "BT_PRV", "BT_PREV", "BT_CLR_ALL"].contains(cmd_raw);

            if is_single_param || val.is_empty() {
                let display_cmd = match *cmd_raw {
                    "BT_CLR" => "CLR",
                    "BT_NXT" | "BT_NEXT" => "NEXT",
                    "BT_PRV" | "BT_PREV" => "PREV",
                    "BT_CLR_ALL" => "CLR ALL",
                    _ => cmd,
                };
                BindingParts {
                    top_left: "bt".into(),
                    top_right: "".into(),
                    center: display_cmd.into(),
                }
            } else {
                BindingParts {
                    top_left: "bt".into(),
                    top_right: cmd.into(),
                    center: val,
                }
            }
        },
        "&out" | "&ext_power" | "&rgb_ug" | "&bl" => {
            let cmd_raw = params.get(0).unwrap_or(&"");
            let prefix = match behavior_raw {
                "&out" => "OUT_",
                "&ext_power" => "EP_",
                "&rgb_ug" => "RGB_",
                "&bl" => "BL_",
                _ => "",
            };
            let cmd = cmd_raw.strip_prefix(prefix).unwrap_or(cmd_raw);
            BindingParts {
                top_left: behavior.into(),
                top_right: "".into(),
                center: cmd.into(),
            }
        },
        _ => {
            // Default: behavior in TL, first param in center if it exists
            BindingParts {
                top_left: behavior.into(),
                top_right: "".into(),
                center: params.get(0).map(|&p| format_keycode(p)).unwrap_or_default(),
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

fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
     .replace("<", "&lt;")
     .replace(">", "&gt;")
     .replace("\"", "&quot;")
     .replace("'", "&apos;")
}

pub fn generate_svg(data: &KeymapData) -> String {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut avg_w = 0.0;
    for pk in &data.physical_layout {
        avg_w += pk.width as f32;
        if pk.x < min_x { min_x = pk.x; }
        if pk.x + pk.width > max_x { max_x = pk.x + pk.width; }
        if pk.y < min_y { min_y = pk.y; }
        if pk.y + pk.height > max_y { max_y = pk.y + pk.height; }
    }
    if data.physical_layout.is_empty() { return String::new(); }
    avg_w /= data.physical_layout.len() as f32;

    let u_size = if avg_w < 500.0 { 100.0 } else { 1000.0 };
    let size_scale = 44.0 / u_size;
    let u_pos = if max_x.abs() > 20000 || min_x.abs() > 20000 { 19050.0 } else { u_size };
    let pos_scale = 44.0 / u_pos;

    let layer_width = (max_x - min_x) as f32 * pos_scale;
    let layer_height = (max_y - min_y) as f32 * pos_scale;
    let padding = 100.0;

    let total_width = layer_width + 80.0;
    let total_height = (layer_height + padding) * data.layers.len() as f32 + 80.0;

    let mut svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" style="background-color: #ffffff;">"#, total_width, total_height, total_width, total_height);
    svg.push_str("<style>text { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace; } .key { fill: white; stroke: #d1d5db; stroke-width: 0.5; } .label { fill: #9ca3af; font-size: 5px; } .main-text { fill: #1f2937; font-size: 8px; font-weight: bold; } .layer-title { font-size: 18px; font-weight: bold; fill: #111827; }</style>");

    for (l_idx, layer) in data.layers.iter().enumerate() {
        let y_offset = (layer_height + padding) * l_idx as f32 + 80.0;
        svg.push_str(&format!(r#"<text x="40" y="{}" class="layer-title">{}</text>"#, y_offset - 25.0, escape_xml(&layer.name)));

        let offset_x = -(min_x as f32 * pos_scale) + 40.0;
        let offset_y = -(min_y as f32 * pos_scale) + y_offset;

        for (i, pk) in data.physical_layout.iter().enumerate() {
            let binding = layer.bindings.get(i).cloned().unwrap_or_else(|| "".to_string());
            let parts = get_binding_parts(&binding);

            let x = pk.x as f32 * pos_scale + offset_x;
            let y = pk.y as f32 * pos_scale + offset_y;
            let w = (pk.width as f32 * size_scale).max(20.0) - 4.0;
            let h = (pk.height as f32 * size_scale).max(20.0) - 4.0;
            let rotation = pk.rotation as f32 / 1000.0;

            svg.push_str(&format!(r#"<g transform="translate({}, {}) rotate({})">"#, x + w/2.0, y + h/2.0, rotation));
            svg.push_str(&format!(r#"<rect x="{}" y="{}" width="{}" height="{}" rx="2" ry="2" class="key" />"#, -w/2.0, -h/2.0, w, h));

            if !parts.top_left.is_empty() {
                svg.push_str(&format!(r#"<text x="{}" y="{}" class="label" text-anchor="start">{}</text>"#, -w/2.0 + 1.5, -h/2.0 + 4.5, escape_xml(&parts.top_left)));
            }
            if !parts.top_right.is_empty() {
                let display_tr = parts.top_right.chars().take(8).collect::<String>();
                svg.push_str(&format!(r#"<text x="0" y="{}" class="label" text-anchor="middle">{}</text>"#, -h/2.0 + 4.5, escape_xml(&display_tr)));
            }

            let center_y = if !parts.top_right.is_empty() { 2.0 } else { 0.0 };
            let display_center = parts.center.chars().take(12).collect::<String>();
            svg.push_str(&format!(r#"<text x="0" y="{}" class="main-text" text-anchor="middle" dominant-baseline="middle">{}</text>"#, center_y, escape_xml(&display_center)));

            svg.push_str("</g>");
        }
    }

    svg.push_str("</svg>");
    svg
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
    let file_handle = use_state(|| None::<FileSystemFileHandle>);

    let on_open = {
        let keymap_data = keymap_data.clone();
        let original_content = original_content.clone();
        let error = error.clone();
        let loading = loading.clone();
        let file_handle = file_handle.clone();
        Callback::from(move |_| {
            let keymap_data = keymap_data.clone();
            let original_content = original_content.clone();
            let error = error.clone();
            let loading = loading.clone();
            let file_handle = file_handle.clone();
            spawn_local(async move {
                let options = js_sys::Object::new();
                let types = js_sys::Array::new();
                let type0 = js_sys::Object::new();
                js_sys::Reflect::set(&type0, &"description".into(), &"ZMK Keymap Files".into()).unwrap();
                let accept = js_sys::Object::new();
                let extensions = js_sys::Array::new();
                extensions.push(&".keymap".into());
                js_sys::Reflect::set(&accept, &"text/plain".into(), &extensions).unwrap();
                js_sys::Reflect::set(&type0, &"accept".into(), &accept).unwrap();
                types.push(&type0);
                js_sys::Reflect::set(&options, &"types".into(), &types).unwrap();
                js_sys::Reflect::set(&options, &"excludeAcceptAllOption".into(), &JsValue::from(true)).unwrap();
                js_sys::Reflect::set(&options, &"multiple".into(), &JsValue::from(false)).unwrap();

                let picker_promise = show_open_file_picker(&options);
                let result = wasm_bindgen_futures::JsFuture::from(picker_promise).await;

                match result {
                    Ok(handles) => {
                        let handles: js_sys::Array = handles.unchecked_into();
                        if handles.length() > 0 {
                            let handle_val = handles.get(0);
                            let handle: FileSystemFileHandle = handle_val.unchecked_into();
                            file_handle.set(Some(handle.clone().unchecked_into()));

                            loading.set(true);
                            let file_promise = handle.get_file();
                            let file_result = wasm_bindgen_futures::JsFuture::from(file_promise).await;

                            match file_result {
                                Ok(file_val) => {
                                    let file: web_sys::File = file_val.unchecked_into();
                                    let content_promise = file.text();
                                    let content_result = wasm_bindgen_futures::JsFuture::from(content_promise).await;

                                    match content_result {
                                        Ok(content_val) => {
                                            let content = content_val.as_string().unwrap_or_default();
                                            original_content.set(content.clone());

                                            let parse_result = Request::post("/api/parse-keymap")
                                                .json(&KeymapRequest { content })
                                                .unwrap()
                                                .send()
                                                .await;

                                            loading.set(false);
                                            match parse_result {
                                                Ok(resp) => {
                                                    if resp.ok() {
                                                        match resp.json::<KeymapData>().await {
                                                            Ok(data) => {
                                                                keymap_data.set(Some(data));
                                                                error.set(None);
                                                            }
                                                            Err(e) => error.set(Some(format!("JSON Parse error: {}", e))),
                                                        }
                                                    } else {
                                                        error.set(Some(format!("Server error: {}", resp.text().await.unwrap_or_default())));
                                                    }
                                                }
                                                Err(e) => error.set(Some(format!("Network error: {}", e))),
                                            }
                                        }
                                        Err(e) => {
                                            loading.set(false);
                                            error.set(Some(format!("Failed to read file content: {:?}", e)));
                                        }
                                    }
                                }
                                Err(e) => {
                                    loading.set(false);
                                    error.set(Some(format!("Failed to get file from handle: {:?}", e)));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("File picker error: {:?}", e)));
                    }
                }
            });
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
        let file_handle = file_handle.clone();
        Callback::from(move |_| {
            if let Some(data) = &*keymap_data {
                let original_content_str = (*original_content).clone();
                let data = data.clone();
                let error = error.clone();
                let loading = loading.clone();
                let file_handle_val = (*file_handle).clone();

                loading.set(true);
                spawn_local(async move {
                    let result = Request::post("/api/save-keymap")
                        .json(&SaveKeymapRequest { original_content: original_content_str, data })
                        .unwrap()
                        .send()
                        .await;

                    match result {
                        Ok(resp) => {
                            if resp.ok() {
                                match resp.json::<SaveKeymapResponse>().await {
                                    Ok(res) => {
                                        // Check if we have a direct file handle
                                        if let Some(handle) = file_handle_val {
                                            let writable_promise = handle.create_writable();
                                            let writable_result = wasm_bindgen_futures::JsFuture::from(writable_promise).await;

                                            match writable_result {
                                                Ok(writable_val) => {
                                                    let writable: FileSystemWritableFileStream = writable_val.unchecked_into();
                                                    let write_promise = writable.write(&JsValue::from_str(&res.content));
                                                    let _ = wasm_bindgen_futures::JsFuture::from(write_promise).await;
                                                    let close_promise = writable.close();
                                                    let _ = wasm_bindgen_futures::JsFuture::from(close_promise).await;
                                                    loading.set(false);
                                                    error.set(None);
                                                }
                                                Err(e) => {
                                                    loading.set(false);
                                                    error.set(Some(format!("Failed to create writable: {:?}", e)));
                                                }
                                            }
                                        } else {
                                            // Fallback to traditional download if handle is missing
                                            loading.set(false);
                                            let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&res.content))).unwrap();
                                            let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                                            let window = web_sys::window().unwrap();
                                            let document = window.document().unwrap();
                                            let link = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                                            link.set_href(&url);
                                            link.set_download("edited.keymap");
                                            link.click();
                                            web_sys::Url::revoke_object_url(&url).unwrap();
                                        }
                                    }
                                    Err(e) => {
                                        loading.set(false);
                                        error.set(Some(format!("Failed to parse server response: {}", e)));
                                    }
                                }
                            } else {
                                loading.set(false);
                                let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                                error.set(Some(format!("Server error: {}", error_text)));
                            }
                        }
                        Err(e) => {
                            loading.set(false);
                            error.set(Some(format!("Network error: {}", e)));
                        }
                    }
                });
            }
        })
    };

    {
        let on_open = on_open.clone();
        let on_save = on_save.clone();
        let has_data = keymap_data.is_some();
        let keymap_data_val = (*keymap_data).clone();
        use_effect_with((has_data, keymap_data_val, (*file_handle).clone()), move |(has_data, _, _)| {
            let on_open = on_open.clone();
            let on_save = on_save.clone();
            let has_data = *has_data;
            let key_listener = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                let key = e.key().to_lowercase();
                if (e.ctrl_key() || e.meta_key()) && key == "o" {
                    e.prevent_default();
                    on_open.emit(MouseEvent::new("click").unwrap());
                } else if (e.ctrl_key() || e.meta_key()) && key == "s" {
                    if has_data {
                        e.prevent_default();
                        on_save.emit(MouseEvent::new("click").unwrap());
                    }
                }
            }) as Box<dyn FnMut(KeyboardEvent)>);

            let window = web_sys::window().expect("should have a window");
            window.add_event_listener_with_callback("keydown", key_listener.as_ref().unchecked_ref()).expect("failed to add listener");
            move || {
                let window = web_sys::window().expect("should have a window");
                window.remove_event_listener_with_callback("keydown", key_listener.as_ref().unchecked_ref()).expect("failed to remove listener");
                drop(key_listener);
            }
        });
    }

    let on_download_svg = {
        let keymap_data = keymap_data.clone();
        let file_handle = file_handle.clone();
        Callback::from(move |_| {
            if let Some(data) = &*keymap_data {
                let svg_content = generate_svg(data);
                let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&svg_content))).unwrap();
                let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                let window = web_sys::window().unwrap();
                let document = window.document().unwrap();
                let link = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                link.set_href(&url);

                let filename = if let Some(handle) = &*file_handle {
                    let mut name = handle.name();
                    if name.ends_with(".keymap") {
                        name = name.replace(".keymap", ".svg");
                    } else {
                        name.push_str(".svg");
                    }
                    name
                } else {
                    "keymap.svg".to_string()
                };
                link.set_download(&filename);

                link.click();
                web_sys::Url::revoke_object_url(&url).unwrap();
            }
        })
    };

    html! {
        <div class="w-full flex flex-col items-center p-4">
            <h2 class="text-4xl font-display mb-8">{"ZMK Keymap Editor"}</h2>

            <div class="flex items-center space-x-4 mb-8">
                <div>
                    <div class="flex flex-col space-y-2">
                        <label class="block text-sm font-medium text-gray-900 dark:text-white">{"Open keymap file"}</label>
                        <button onclick={on_open.clone()} class="px-6 py-2.5 bg-blue-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-blue-700 hover:shadow-lg focus:bg-blue-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-blue-800 active:shadow-lg transition duration-150 ease-in-out">
                            {"Open File"}
                        </button>
                    </div>
                    <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">{"Type "} <kbd class="px-1.5 py-0.5 font-sans font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">{"j"}</kbd> {" to start jump mode"}</p>
                </div>
                { if keymap_data.is_some() {
                    html! {
                        <div class="flex space-x-2 mt-6">
                            <button onclick={on_save.clone()} class="px-6 py-2.5 bg-green-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-green-700 hover:shadow-lg focus:bg-green-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-green-800 active:shadow-lg transition duration-150 ease-in-out">
                                {"Save Keymap"}
                            </button>
                            <button onclick={on_download_svg.clone()} class="px-6 py-2.5 bg-purple-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-purple-700 hover:shadow-lg focus:bg-purple-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-purple-800 active:shadow-lg transition duration-150 ease-in-out">
                                {"Download SVG"}
                            </button>
                        </div>
                    }
                } else { html! {} }}
            </div>

            { if *loading {
                html! { <div class="text-blue-500 mb-4 animate-pulse">{"Processing..."}</div> }
            } else { html! {} }}

            { if let Some(err) = &*error {
                html! { <div class="text-red-500 mb-4">{err}</div> }
            } else { html! {} }}

            { if let Some(data) = &*keymap_data {
                let on_update_data_clone = on_update_data.clone();
                html! { <KeymapRenderer data={data.clone()} on_update={on_update_data_clone} /> }
            } else {
                if !*loading { html! { <div class="text-gray-500 italic">{"Please open a keymap file to start editing."}</div> } } else { html! {} }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct RendererProps {
    pub data: KeymapData,
    pub on_update: Callback<KeymapData>,
}

#[derive(Clone, PartialEq, Debug)]
enum HintTarget {
    Key(usize),
    Layer(usize),
    LayerMenu(usize),
    Menu(usize, usize),
}

#[function_component]
fn KeymapRenderer(props: &RendererProps) -> Html {
    let current_layer = use_state(|| 0);
    let selected_key = use_state(|| None::<SelectedKey>);
    let show_param_selection = use_state(|| false);

    let jump_mode_active = use_state(|| false);
    let jump_input = use_state(|| String::new());
    let container_ref = use_node_ref();

    let layer_menu_index = use_state(|| None::<usize>);
    let menu_focus_index = use_state(|| 0usize);
    let quick_assign_index = use_state(|| None::<usize>);

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

    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut avg_w = 0.0;
    for pk in &props.data.physical_layout {
        avg_w += pk.width as f32;
        if pk.x < min_x { min_x = pk.x; }
        if pk.x + pk.width > max_x { max_x = pk.x + pk.width; }
        if pk.y < min_y { min_y = pk.y; }
        if pk.y + pk.height > max_y { max_y = pk.y + pk.height; }
    }
    if !props.data.physical_layout.is_empty() {
        avg_w /= props.data.physical_layout.len() as f32;
    }

    let u_size = if avg_w < 500.0 { 100.0 } else { 1000.0 };
    let size_scale = 44.0 / u_size;
    let u_pos = if max_x.abs() > 20000 || min_x.abs() > 20000 { 19050.0 } else { u_size };
    let pos_scale = 44.0 / u_pos;

    let content_width_px = (max_x - min_x) as f32 * pos_scale;
    let offset_x = -(min_x as f32 * pos_scale);

    let on_key_click = {
        let selected_key = selected_key.clone();
        let current_layer = current_layer.clone();
        let quick_assign_index = quick_assign_index.clone();
        Callback::from(move |key_index: usize| {
            if quick_assign_index.is_some() {
                quick_assign_index.set(Some(key_index));
            } else {
                selected_key.set(Some(SelectedKey {
                    layer_index: *current_layer,
                    key_index,
                }));
            }
        })
    };

    let update_layer_batch = {
        let data = props.data.clone();
        let on_update = props.on_update.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize, from: &str, to: &str| {
            let mut new_data = data.clone();
            for binding in new_data.layers[idx].bindings.iter_mut() {
                if binding == from { *binding = to.to_string(); }
            }
            on_update.emit(new_data);
            layer_menu_index.set(None);
        }
    };

    let duplicate_layer = {
        let data = props.data.clone();
        let on_update = props.on_update.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let mut new_data = data.clone();
            let mut new_layer = new_data.layers[idx].clone();
            new_layer.name = format!("{} (copy)", new_layer.name);
            new_data.layers.insert(idx + 1, new_layer);
            on_update.emit(new_data);
            layer_menu_index.set(None);
        }
    };

    let delete_layer = {
        let data = props.data.clone();
        let on_update = props.on_update.clone();
        let layer_menu_index = layer_menu_index.clone();
        let current_layer = current_layer.clone();
        move |idx: usize| {
            if data.layers.len() <= 1 { return; }
            let mut new_data = data.clone();
            new_data.layers.remove(idx);
            if *current_layer >= new_data.layers.len() { current_layer.set(new_data.layers.len() - 1); }
            on_update.emit(new_data);
            layer_menu_index.set(None);
        }
    };

    let move_layer = {
        let data = props.data.clone();
        let on_update = props.on_update.clone();
        let layer_menu_index = layer_menu_index.clone();
        let current_layer = current_layer.clone();
        move |idx: usize, up: bool| {
            if up && idx == 0 { return; }
            if !up && idx == data.layers.len() - 1 { return; }
            let mut new_data = data.clone();
            let target = if up { idx - 1 } else { idx + 1 };
            new_data.layers.swap(idx, target);
            if *current_layer == idx { current_layer.set(target); }
            else if *current_layer == target { current_layer.set(idx); }
            on_update.emit(new_data);
            layer_menu_index.set(None);
        }
    };

    let rename_layer = {
        let data = props.data.clone();
        let on_update = props.on_update.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let window = web_sys::window().unwrap();
            if let Ok(Some(new_name)) = window.prompt_with_message_and_default("Rename layer:", &data.layers[idx].name) {
                if !new_name.trim().is_empty() {
                    let mut new_data = data.clone();
                    new_data.layers[idx].name = new_name.trim().to_string();
                    on_update.emit(new_data);
                }
            }
            layer_menu_index.set(None);
        }
    };

    let reset_layer = {
        let data = props.data.clone();
        let on_update = props.on_update.clone();
        let layer_menu_index = layer_menu_index.clone();
        move |idx: usize| {
            let mut new_data = data.clone();
            for binding in new_data.layers[idx].bindings.iter_mut() { *binding = "&none".to_string(); }
            on_update.emit(new_data);
            layer_menu_index.set(None);
        }
    };

    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let mut all_targets = Vec::new();
    for i in 0..props.data.layers.len() {
        all_targets.push(HintTarget::Layer(i));
        all_targets.push(HintTarget::LayerMenu(i));
        if let Some(lmi) = *layer_menu_index {
            if lmi == i { for j in 0..9 { all_targets.push(HintTarget::Menu(i, j)); } }
        }
    }
    let mut key_indices: Vec<usize> = (0..props.data.physical_layout.len()).collect();
    key_indices.sort_by(|&a, &b| {
        let ka = &props.data.physical_layout[a];
        let kb = &props.data.physical_layout[b];
        (ka.y / 10).cmp(&(kb.y / 10)).then(ka.x.cmp(&kb.x))
    });
    for &idx in &key_indices { all_targets.push(HintTarget::Key(idx)); }

    let mut hint_map = std::collections::HashMap::new();
    let mut hints = vec![String::new(); props.data.physical_layout.len()];
    for (i, target) in all_targets.into_iter().enumerate() {
        if i < hint_chars.len() * hint_chars.len() {
            let h = format!("{}{}", hint_chars.chars().nth(i / hint_chars.len()).unwrap(), hint_chars.chars().nth(i % hint_chars.len()).unwrap());
            if let HintTarget::Key(idx) = target { hints[idx] = h.clone(); }
            hint_map.insert(h, target);
        }
    }

    let on_keydown = {
        let jump_mode_active = jump_mode_active.clone();
        let jump_input = jump_input.clone();
        let selected_key = selected_key.clone();
        let on_key_click = on_key_click.clone();
        let hint_map_c = hint_map.clone();
        let quick_assign_index = quick_assign_index.clone();
        let on_update = props.on_update.clone();
        let data = props.data.clone();
        let current_layer = current_layer.clone();
        let layer_menu_index = layer_menu_index.clone();
        let menu_focus_index = menu_focus_index.clone();
        let move_l = move_layer.clone();
        let dup_l = duplicate_layer.clone();
        let del_l = delete_layer.clone();
        let ren_l = rename_layer.clone();
        let res_l = reset_layer.clone();
        let batch_t_n = update_layer_batch.clone();
        let batch_n_t = update_layer_batch.clone();

        Callback::from(move |e: KeyboardEvent| {
            if let Some(l_idx) = *layer_menu_index {
                match e.key().as_str() {
                    "ArrowDown" => { menu_focus_index.set((*menu_focus_index + 1) % 9); e.prevent_default(); return; }
                    "ArrowUp" => { menu_focus_index.set((*menu_focus_index + 8) % 9); e.prevent_default(); return; }
                    "Enter" => {
                        match *menu_focus_index {
                            0 => move_l(l_idx, true), 1 => move_l(l_idx, false), 2 => ren_l(l_idx), 3 => dup_l(l_idx),
                            4 => del_l(l_idx), 5 => res_l(l_idx), 6 => batch_t_n(l_idx, "&trans", "&none"),
                            7 => batch_n_t(l_idx, "&none", "&trans"), 8 => { quick_assign_index.set(Some(0)); layer_menu_index.set(None); }
                            _ => {}
                        }
                        e.prevent_default(); return;
                    }
                    "Escape" => { layer_menu_index.set(None); e.prevent_default(); return; }
                    _ => {}
                }
            }
            if selected_key.is_some() { return; }
            if let Some(idx) = *quick_assign_index {
                if e.key() == "Escape" { quick_assign_index.set(None); e.prevent_default(); return; }
                if let Some(zmk_key) = keycodes::to_zmk_keycode(&e.key()) {
                    let mut new_data = data.clone();
                    new_data.layers[*current_layer].bindings[idx] = format!("&kp {}", zmk_key);
                    quick_assign_index.set(Some((idx + 1) % data.physical_layout.len()));
                    on_update.emit(new_data); e.prevent_default(); return;
                }
            }
            if *jump_mode_active {
                match e.key().as_str() {
                    "Enter" | "Escape" => { jump_mode_active.set(false); jump_input.set(String::new()); e.prevent_default(); }
                    key if key.len() == 1 && hint_chars.contains(key) => {
                        let mut new_input = (*jump_input).clone(); new_input.push_str(key);
                        if let Some(target) = hint_map_c.get(&new_input) {
                            match target {
                                HintTarget::Key(idx) => on_key_click.emit(*idx),
                                HintTarget::Layer(idx) => current_layer.set(*idx),
                                HintTarget::LayerMenu(idx) => { layer_menu_index.set(Some(*idx)); menu_focus_index.set(0); }
                                HintTarget::Menu(l_idx, m_idx) => {
                                    match *m_idx {
                                        0 => move_l(*l_idx, true), 1 => move_l(*l_idx, false), 2 => ren_l(*l_idx),
                                        3 => dup_l(*l_idx), 4 => del_l(*l_idx), 5 => res_l(*l_idx),
                                        6 => batch_t_n(*l_idx, "&trans", "&none"), 7 => batch_n_t(*l_idx, "&none", "&trans"),
                                        8 => { quick_assign_index.set(Some(0)); layer_menu_index.set(None); }
                                        _ => {}
                                    }
                                }
                            }
                            jump_mode_active.set(false); jump_input.set(String::new());
                        } else if hint_map_c.keys().any(|h| h.starts_with(&new_input)) { jump_input.set(new_input); }
                        e.prevent_default();
                    }
                    _ => {}
                }
            } else if e.key() == "j" { jump_mode_active.set(true); jump_input.set(String::new()); e.prevent_default(); }
        })
    };

    let close_popup = {
        let selected_key = selected_key.clone();
        let show_param_selection = show_param_selection.clone();
        let container_ref = container_ref.clone();
        Callback::from(move |_: MouseEvent| {
            selected_key.set(None); show_param_selection.set(false);
            if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() { let _ = element.focus(); }
        })
    };

    let toggle_param_selection = {
        let show_param_selection = show_param_selection.clone();
        Callback::from(move |_: MouseEvent| show_param_selection.set(!*show_param_selection))
    };

    {
        let layer_menu_index = layer_menu_index.clone();
        use_effect(move || {
            let lmi = layer_menu_index.clone();
            let click_listener = Closure::wrap(Box::new(move |_e: MouseEvent| lmi.set(None)) as Box<dyn FnMut(MouseEvent)>);
            let lmi_esc = layer_menu_index.clone();
            let key_listener = Closure::wrap(Box::new(move |e: KeyboardEvent| if e.key() == "Escape" { lmi_esc.set(None); }) as Box<dyn FnMut(KeyboardEvent)>);
            let window = web_sys::window().unwrap();
            window.add_event_listener_with_callback("click", click_listener.as_ref().unchecked_ref()).unwrap();
            window.add_event_listener_with_callback("keydown", key_listener.as_ref().unchecked_ref()).unwrap();
            move || {
                let window = web_sys::window().unwrap();
                window.remove_event_listener_with_callback("click", click_listener.as_ref().unchecked_ref()).unwrap();
                window.remove_event_listener_with_callback("keydown", key_listener.as_ref().unchecked_ref()).unwrap();
                drop(click_listener); drop(key_listener);
            }
        });
    }

    html! {
        <div ref={container_ref} tabindex="0" onkeydown={on_keydown} class="flex flex-col items-center w-full mt-4 focus:outline-none">
            <div class={classes!("flex", "flex-wrap", "justify-center", "gap-2", "mb-8", "bg-gray-100", "dark:bg-gray-800", "p-2", "rounded-2xl", "shadow-inner", "border", "border-gray-200", "dark:border-gray-700")}>
                { for props.data.layers.iter().enumerate().map(|(i, l)| {
                    let is_active = i == *current_layer;
                    let is_menu_open = *layer_menu_index == Some(i);
                    let ontogglemenu = {
                        let layer_menu_index = layer_menu_index.clone();
                        Callback::from(move |e: MouseEvent| {
                            e.stop_propagation();
                            if *layer_menu_index == Some(i) { layer_menu_index.set(None); } else { layer_menu_index.set(Some(i)); }
                        })
                    };
                    let class = if is_active { "px-6 py-2 bg-blue-500 text-white font-semibold rounded-xl shadow-md transition-all duration-200 flex items-center gap-2" } else { "px-6 py-2 bg-transparent text-gray-600 dark:text-gray-400 font-medium rounded-xl hover:bg-gray-200 dark:hover:bg-gray-700 transition-all duration-200 cursor-pointer flex items-center gap-2" };
                    let layer_hint = hint_map.iter().find(|(_, t)| **t == HintTarget::Layer(i)).map(|(h, _)| h);
                    let show_layer_hint = *jump_mode_active && layer_hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);
                    let menu_trigger_hint = hint_map.iter().find(|(_, t)| **t == HintTarget::LayerMenu(i)).map(|(h, _)| h);
                    let show_menu_trigger_hint = *jump_mode_active && menu_trigger_hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);
                    html! {
                        <div class="relative" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                            <button onclick={let cl = current_layer.clone(); Callback::from(move |_| cl.set(i))} class={classes!(class.split_whitespace().collect::<Vec<_>>())}>
                                {&l.name}
                                <span onclick={ontogglemenu} class="hover:bg-black/10 rounded p-1 relative">
                                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                                    { if show_menu_trigger_hint {
                                        let h = menu_trigger_hint.unwrap();
                                        let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                        html! { <div class="absolute -top-2 -right-2 bg-blue-400 dark:bg-blue-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                    } else { html! {} }}
                                </span>
                                { if show_layer_hint {
                                    let h = layer_hint.unwrap();
                                    let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                    html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-tl-xl rounded-br-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                } else { html! {} }}
                            </button>
                            { if is_menu_open {
                                let move_up = move_layer.clone(); let move_dn = move_layer.clone(); let dup = duplicate_layer.clone(); let del = delete_layer.clone(); let ren = rename_layer.clone(); let res = reset_layer.clone();
                                let batch_t_n = update_layer_batch.clone(); let batch_n_t = update_layer_batch.clone(); let qa = quick_assign_index.clone();
                                let menu_items = vec![
                                    ("Move Up", Callback::from(move |_| move_up(i, true))), ("Move Down", Callback::from(move |_| move_dn(i, false))), ("Rename", Callback::from(move |_| ren(i))), ("Duplicate", Callback::from(move |_| dup(i))),
                                    ("Delete", Callback::from(move |_| del(i))), ("Reset all to None", Callback::from(move |_| res(i))), ("Trans → None", Callback::from(move |_| batch_t_n(i, "&trans", "&none"))),
                                    ("None → Trans", Callback::from(move |_| batch_n_t(i, "&none", "&trans"))), ("Quick &kp Assignment", { let qa = qa.clone(); let lmi = layer_menu_index.clone(); Callback::from(move |_| { qa.set(Some(0)); lmi.set(None); }) }),
                                ];
                                html! {
                                    <div class="absolute top-full left-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 z-50 py-1 overflow-hidden">
                                        { for menu_items.into_iter().enumerate().map(|(j, (label, cb))| {
                                            let is_focused = *menu_focus_index == j;
                                            let menu_hint = hint_map.iter().find(|(_, t)| **t == HintTarget::Menu(i, j)).map(|(h, _)| h);
                                            let show_menu_hint = *jump_mode_active && menu_hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);
                                            let class = classes!("w-full", "text-left", "px-4", "py-2", "text-sm", "relative", if is_focused { "bg-blue-100 dark:bg-blue-900/40" } else { "hover:bg-gray-100 dark:hover:bg-gray-700" }, if j == 4 { "text-red-500" } else if j == 5 { "text-orange-500" } else if j == 8 { "font-bold text-blue-500" } else { "" });
                                            html! { <> { if j == 5 || j == 8 { html! { <div class="border-t border-gray-200 dark:border-gray-700 my-1"></div> } } else { html! {} } }
                                                <button onclick={cb} class={class}> {label}
                                                    { if show_menu_hint {
                                                        let h = menu_hint.unwrap();
                                                        let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                                        html! { <div class="absolute top-0 right-0 bg-yellow-400 dark:bg-yellow-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-bl-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                                    } else { html! {} }}
                                                </button> </>
                                            }
                                        })}
                                    </div>
                                }
                            } else { html! {} }}
                        </div>
                    }
                })}
            </div>
            { if let Some(idx) = *quick_assign_index {
                let on_vk_click = {
                    let on_update = props.on_update.clone(); let data = props.data.clone(); let current_layer_idx = *current_layer; let quick_assign_index = quick_assign_index.clone();
                    Callback::from(move |zmk_key: String| {
                        let mut new_data = data.clone(); new_data.layers[current_layer_idx].bindings[idx] = format!("&kp {}", zmk_key);
                        quick_assign_index.set(Some((idx + 1) % data.physical_layout.len())); on_update.emit(new_data);
                    })
                };
                html! {
                    <div class="w-full max-w-5xl mb-8 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-xl border border-blue-200 dark:border-blue-800">
                        <div class="flex justify-between items-center mb-4">
                            <div><h3 class="text-lg font-bold text-blue-800 dark:text-blue-300">{"Quick &kp Assignment Mode"}</h3><p class="text-sm text-blue-600 dark:text-blue-400">{"Type on your keyboard or click the virtual keys below. Advances automatically."}</p></div>
                            <button onclick={let qa = quick_assign_index.clone(); Callback::from(move |_| qa.set(None))} class="bg-blue-500 text-white px-4 py-1 rounded-lg font-bold">{"Done"}</button>
                        </div>
                        <VirtualKeyboard on_click={on_vk_click} />
                    </div>
                }
            } else { html! {} }}
            <div class={classes!("relative", "border", "dark:border-gray-600", "p-8", "rounded-xl", "bg-gray-50", "dark:bg-gray-800", "shadow-inner", "overflow-auto", "w-full", "max-w-full")} style="min-height: 350px; height: 55vh;">
                <div class="relative mx-auto" style={format!("width: {}px;", content_width_px)}>
                { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| {
                    let binding = layer.bindings.get(i).cloned().unwrap_or_else(|| "".to_string());
                    let parts = get_binding_parts(&binding);
                    let x = (pk.x as f32 * pos_scale + offset_x) as i32; let y = (pk.y as f32 * pos_scale) as i32;
                    let w = (pk.width as f32 * size_scale).max(20.0) as i32 - 4; let h = (pk.height as f32 * size_scale).max(20.0) as i32 - 4;
                    let rotation_deg = pk.rotation as f32 / 1000.0;
                    let style = format!("left: {}px; top: {}px; width: {}px; height: {}px; transform: rotate({}deg);", x, y, w, h, rotation_deg);
                    let onclick = { let on_key_click = on_key_click.clone(); Callback::from(move |_| on_key_click.emit(i)) };
                    let hint = hints.get(i);
                    let show_hint = *jump_mode_active && hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);
                    let is_quick_assign_target = quick_assign_index.map(|idx| idx == i).unwrap_or(false);
                    html! {
                        <div onclick={onclick} class={classes!("absolute", "bg-white", "dark:bg-gray-700", "border", "border-gray-300", "dark:border-gray-500", "flex", "items-center", "justify-center", "font-mono", "rounded-md", "shadow-sm", if is_quick_assign_target { vec!["ring-4", "ring-blue-500", "z-40"] } else { vec!["hover:border-blue-500"] }, "cursor-pointer", "overflow-hidden", "text-center")} style={style} title={binding.clone()}>
                            { if !parts.top_left.is_empty() { html! { <span class="absolute top-0.5 left-0.5 text-[6px] text-gray-400 leading-none">{&parts.top_left}</span> } } else { html! {} }}
                            { if !parts.top_right.is_empty() { html! { <span class="absolute top-0.5 left-0 right-0 text-[8px] text-gray-400 leading-none text-center">{&parts.top_right}</span> } } else { html! {} }}
                            <span class={classes!("truncate", "px-1", "pointer-events-none", "text-[10px]", if !parts.top_right.is_empty() { "pt-2" } else { "" })}>{&parts.center}</span>
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
            { if let Some(sk) = &*selected_key {
                html! { <KeyBindingPopup data={props.data.clone()} selected_key={sk.clone()} on_close={close_popup.clone()} show_param_selection={*show_param_selection} on_toggle_param_selection={toggle_param_selection.clone()} on_update={props.on_update.clone()} /> }
            } else { html! {} }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct VirtualKeyboardProps { pub on_click: Callback<String>, }

#[function_component]
fn VirtualKeyboard(props: &VirtualKeyboardProps) -> Html {
    let rows = vec![
        vec![("ESC", "ESC", 1.0), ("1", "N1", 1.0), ("2", "N2", 1.0), ("3", "N3", 1.0), ("4", "N4", 1.0), ("5", "N5", 1.0), ("6", "N6", 1.0), ("7", "N7", 1.0), ("8", "N8", 1.0), ("9", "N9", 1.0), ("0", "N0", 1.0), ("-", "MINUS", 1.0), ("=", "EQUAL", 1.0), ("BSPC", "BSPC", 2.0)],
        vec![("TAB", "TAB", 1.5), ("Q", "Q", 1.0), ("W", "W", 1.0), ("E", "E", 1.0), ("R", "R", 1.0), ("T", "T", 1.0), ("Y", "Y", 1.0), ("U", "U", 1.0), ("I", "I", 1.0), ("O", "O", 1.0), ("P", "P", 1.0), ("[", "LBKT", 1.0), ("]", "RBKT", 1.0), ("\\", "BSLH", 1.5)],
        vec![("CAPS", "CAPS", 1.75), ("A", "A", 1.0), ("S", "S", 1.0), ("D", "D", 1.0), ("F", "F", 1.0), ("G", "G", 1.0), ("H", "H", 1.0), ("J", "J", 1.0), ("K", "K", 1.0), ("L", "L", 1.0), (";", "SEMI", 1.0), ("'", "SQT", 1.0), ("ENTER", "ENTER", 2.25)],
        vec![("LSHFT", "LSHFT", 2.25), ("Z", "Z", 1.0), ("X", "X", 1.0), ("C", "C", 1.0), ("V", "V", 1.0), ("B", "B", 1.0), ("N", "N", 1.0), ("M", "M", 1.0), (",", "COMMA", 1.0), (".", "DOT", 1.0), ("/", "SLASH", 1.0), ("RSHFT", "RSHFT", 2.75)],
        vec![("LCTRL", "LCTRL", 1.25), ("LGUI", "LGUI", 1.25), ("LALT", "LALT", 1.25), ("SPACE", "SPACE", 6.25), ("RALT", "RALT", 1.25), ("RGUI", "RGUI", 1.25), ("MENU", "K_APP", 1.25), ("RCTRL", "RCTRL", 1.25)],
    ];
    html! { <div class="flex flex-col gap-1 select-none p-2 bg-gray-200 dark:bg-gray-800 rounded-lg shadow-inner"> { for rows.iter().map(|row| html! { <div class="flex gap-1 justify-center"> { for row.iter().map(|(label, code, size)| {
        let code_c = code.to_string(); let onclick = { let on_click = props.on_click.clone(); Callback::from(move |_| on_click.emit(code_c.clone())) };
        let style = format!("flex-grow: {}; flex-basis: 0;", size);
        html! { <button onclick={onclick} style={style} class="h-10 px-1 bg-white dark:bg-gray-700 hover:bg-blue-500 hover:text-white dark:hover:bg-blue-600 rounded shadow-sm text-xs font-bold transition-colors border border-gray-300 dark:border-gray-600"> {label} </button> }
    })} </div> })} </div> }
}

fn get_keycode_suggestions(query: &str, only_regular: bool, is_tap_param: bool, only_mods: bool) -> Vec<Suggestion> {
    let query = query.to_uppercase();
    let mut results = Vec::new();

    if !only_mods {
        for c in 'A'..='Z' {
            let k = c.to_string();
            if k.contains(&query) {
                results.push(Suggestion { value: k.clone(), display: k });
            }
        }
        for i in 0..=9 {
            let k = format!("N{}", i);
            if k.contains(&query) {
                results.push(Suggestion { value: k.clone(), display: k });
            }
        }
        for i in 1..=24 {
            let k = format!("F{}", i);
            if k.contains(&query) {
                results.push(Suggestion { value: k.clone(), display: k });
            }
        }
    }

    for (&k, &v) in keycodes::KEY_ALIASES.iter() {
        if only_mods && !keycodes::is_modifier(k) { continue; }
        let include = if is_tap_param {
            keycodes::is_regular_key(k) && !keycodes::is_modifier(k)
        } else if only_regular {
            keycodes::is_regular_key(k)
        } else {
            true
        };
        if include && (k.to_uppercase().contains(&query) || v.to_uppercase().contains(&query)) {
            let val = k.to_string();
            let disp = if k != v { format!("{} ({})", k, v) } else { val.clone() };
            results.push(Suggestion { value: val, display: disp });
        }
    }
    results
}

#[derive(Properties, PartialEq)]
pub struct PopupProps { pub data: KeymapData, pub selected_key: SelectedKey, pub on_close: Callback<MouseEvent>, pub show_param_selection: bool, pub on_toggle_param_selection: Callback<MouseEvent>, pub on_update: Callback<KeymapData>, }

#[derive(Serialize)] struct SaveKeymapRequest { original_content: String, data: KeymapData, }
#[derive(Deserialize)] struct SaveKeymapResponse { content: String, }
#[derive(Clone, PartialEq, Debug)] struct Suggestion { value: String, display: String, }

#[function_component]
fn KeyBindingPopup(props: &PopupProps) -> Html {
    let filter = use_state(|| String::new());
    let suggestion_container_ref = use_node_ref();
    let input_ref = use_node_ref();
    let binding = &props.data.layers[props.selected_key.layer_index].bindings[props.selected_key.key_index];
    {
        let input_ref = input_ref.clone();
        use_effect_with(props.selected_key.clone(), move |_| {
            let timeout_cb = Closure::wrap(Box::new(move || { if let Some(input) = input_ref.cast::<web_sys::HtmlInputElement>() { let _ = input.focus(); let _ = input.select(); } }) as Box<dyn FnMut()>);
            let window = web_sys::window().expect("should have a window");
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(timeout_cb.as_ref().unchecked_ref(), 50);
            move || { drop(timeout_cb); }
        });
    }
    let parts: Vec<&str> = binding.split_whitespace().collect();
    let initial_behavior_label = parts.get(0).cloned().unwrap_or("");
    let initial_params = parts[1..].iter().map(|&s| s.to_string()).collect::<Vec<String>>();
    let current_text = use_state(|| binding.clone());
    let current_behavior_label = use_state(|| initial_behavior_label.to_string());
    let current_params = use_state(|| initial_params.clone());
    let show_behavior_selection = use_state(|| false);
    let suggestion_index = use_state(|| 0usize);
    let show_suggestions = use_state(|| false);
    let behavior_name = current_behavior_label.strip_prefix('&').unwrap_or(&*current_behavior_label);
    let behavior_meta = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name);
    let display_name = behavior_meta.and_then(|m| m.display_name).unwrap_or(behavior_name);
    let selected_param_idx = use_state(|| 0usize);
    let is_modifier_only_param = |behavior_name: &str, param_idx: usize| behavior_name == "sk" || (behavior_name == "mt" && param_idx == 0);
    let get_expected_param_count = |behavior_label: &str, params: &[String]| -> usize {
        let behavior_name = behavior_label.strip_prefix('&').unwrap_or(behavior_label);
        if let Some(meta) = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name) {
            if behavior_name == "bt" { if let Some(cmd) = params.get(0) { if cmd == "BT_SEL" || cmd == "BT_DISC" { return 2; } else { return 1; } } return 1; }
            return meta.binding_cells as usize;
        }
        params.len()
    };
    let expected_p_count = get_expected_param_count(&*current_behavior_label, &*current_params);
    let is_valid = {
        if let Some(meta) = behavior_meta {
            if current_params.len() != expected_p_count { false } else {
                let mut all_valid = true;
                for (i, p) in current_params.iter().enumerate() {
                    if p == "UNKNOWN" { all_valid = false; break; }
                    if let Some(ptype) = meta.parameter_metadata.get(i) {
                        match ptype { ParameterType::Layer => { if p.parse::<usize>().is_err() && !props.data.layers.iter().any(|l| &l.name == p) { all_valid = false; break; } } _ => {} }
                    }
                }
                all_valid
            }
        } else { false }
    };
    let update_from_text = {
        let current_text = current_text.clone(); let current_behavior_label = current_behavior_label.clone(); let current_params = current_params.clone(); let show_suggestions = show_suggestions.clone();
        Callback::from(move |text: String| {
            current_text.set(text.clone()); let parts: Vec<&str> = text.split_whitespace().collect();
            current_behavior_label.set(parts.get(0).map(|&s| s.to_string()).unwrap_or_default());
            current_params.set(parts[1..].iter().map(|&s| s.to_string()).collect::<Vec<String>>());
            show_suggestions.set(true);
        })
    };
    let on_apply = {
        let on_update = props.on_update.clone(); let data = props.data.clone(); let selected_key = props.selected_key.clone();
        let current_behavior_label = current_behavior_label.clone(); let current_params = current_params.clone(); let on_close = props.on_close.clone();
        Callback::from(move |e: MouseEvent| {
            if !is_valid { return; }
            let mut new_data = data.clone(); let mut new_binding = (*current_behavior_label).clone();
            for p in &*current_params { new_binding.push(' '); new_binding.push_str(p); }
            new_data.layers[selected_key.layer_index].bindings[selected_key.key_index] = new_binding;
            let behavior_name = current_behavior_label.strip_prefix('&').unwrap_or(&*current_behavior_label);
            if let Some(meta) = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name) {
                if !meta.is_default && !meta.include_file.is_empty() && !new_data.includes.iter().any(|i| i == meta.include_file) { new_data.includes.push(meta.include_file.to_string()); }
            }
            on_update.emit(new_data); on_close.emit(e);
        })
    };
    let select_behavior = {
        let current_behavior_label = current_behavior_label.clone(); let current_params = current_params.clone(); let current_text = current_text.clone(); let show_behavior_selection = show_behavior_selection.clone();
        Callback::from(move |b: &'static behaviors::ZmkBehavior| {
            let label = format!("&{}", b.label.unwrap_or(b.name)); current_behavior_label.set(label.clone());
            let new_params = vec!["UNKNOWN".to_string(); b.binding_cells as usize]; current_params.set(new_params.clone());
            let mut text = label; for p in &new_params { text.push(' '); text.push_str(p); }
            current_text.set(text); show_behavior_selection.set(false);
        })
    };
    let select_param_value = {
        let current_params = current_params.clone(); let selected_param_idx = selected_param_idx.clone(); let current_text = current_text.clone(); let current_behavior_label = current_behavior_label.clone();
        let on_toggle_param_selection = props.on_toggle_param_selection.clone(); let behavior_name_str = behavior_name.to_string();
        Callback::from(move |val: String| {
            let mut new_params = (*current_params).clone(); if let Some(p) = new_params.get_mut(*selected_param_idx) { *p = val.clone(); }
            if behavior_name_str == "bt" && *selected_param_idx == 0 { if val == "BT_SEL" || val == "BT_DISC" { if new_params.len() < 2 { new_params.push("0".to_string()); } } else { if new_params.len() > 1 { new_params.truncate(1); } } }
            current_params.set(new_params.clone());
            let mut text = (*current_behavior_label).clone(); for p in &new_params { text.push(' '); text.push_str(p); }
            current_text.set(text); on_toggle_param_selection.emit(MouseEvent::new("click").unwrap());
        })
    };
    let get_suggestions = {
        let text = (*current_text).clone(); let props_data = props.data.clone();
        move || -> Vec<Suggestion> {
            let parts: Vec<&str> = text.split_whitespace().collect(); let has_trailing_space = text.ends_with(' ');
            let mut results: Vec<Suggestion> = if text.is_empty() || (text == "&" && !has_trailing_space) {
                let mut bh_results: Vec<Suggestion> = ZMK_BEHAVIORS.iter().map(|b| { let val = format!("&{}", b.label.unwrap_or(b.name)); let disp = if let Some(dn) = b.display_name { format!("{} ({})", val, dn) } else { val.clone() }; Suggestion { value: val, display: disp } }).collect();

                // Also include &kp suggestions for alphabetic keys when empty/&
                for c in 'A'..='Z' {
                    let k = c.to_string();
                    bh_results.push(Suggestion { value: format!("&kp {}", k), display: format!("&kp {}", k) });
                }
                bh_results
            } else if !has_trailing_space && parts.len() == 1 {
                let query = parts[0].to_uppercase();
                let mut bh_results: Vec<Suggestion> = ZMK_BEHAVIORS.iter().map(|b| {
                    let val = format!("&{}", b.label.unwrap_or(b.name));
                    let disp = if let Some(dn) = b.display_name { format!("{} ({})", val, dn) } else { val.clone() };
                    Suggestion { value: val, display: disp }
                }).filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query)).collect();

                let mut kp_results = Vec::new();
                for s in get_keycode_suggestions(&query, true, false, false) {
                    kp_results.push(Suggestion { value: format!("&kp {}", s.value), display: format!("&kp {}", s.display) });
                }
                bh_results.append(&mut kp_results);
                bh_results
            } else {
                let p_idx = if has_trailing_space { parts.len() - 1 } else { parts.len() - 2 }; let query = if has_trailing_space { "" } else { parts.last().unwrap_or(&"") }.to_uppercase();
                if let Some(meta) = behavior_meta {
                    if let Some(p_type) = meta.parameter_metadata.get(p_idx) {
                        match p_type {
                            ParameterType::Layer => props_data.layers.iter().enumerate().map(|(i, l)| Suggestion { value: i.to_string(), display: format!("{} ({})", i, l.name) }).filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query)).collect(),
                            ParameterType::Modifier => get_keycode_suggestions(&query, true, false, true),
                            ParameterType::Keycode => {
                                let behavior_name = behavior_meta.map(|m| m.label.unwrap_or(m.name)).unwrap_or("");
                                let is_hold_tap = behavior_meta.map(|m| m.compatible == Some("zmk,behavior-hold-tap")).unwrap_or(false);
                                let only_regular = behavior_name == "kp" || behavior_name == "kt" || is_hold_tap;
                                let is_tap_param = is_hold_tap && p_idx == 1;
                                get_keycode_suggestions(&query, only_regular, is_tap_param, false)
                            },
                            ParameterType::Constant => {
                                let behavior_name = behavior_meta.map(|m| m.label.unwrap_or(m.name)).unwrap_or("");
                                let mut constants_list: Vec<(String, String)> = Vec::new();

                                if let Some(meta) = behavior_meta {
                                    if !meta.constants.is_empty() {
                                        constants_list = meta.constants.iter().map(|&k| (k.to_string(), k.to_string())).collect();
                                    }
                                }
                                if constants_list.is_empty() {
                                    constants_list = keycodes::KEY_ALIASES.iter().map(|(&k, &v)| (k.to_string(), v.to_string())).collect();
                                }

                                let mut results: Vec<Suggestion> = constants_list.into_iter().filter(|(k, _)| match behavior_name {
                                    "mkp" => ["LCLK", "RCLK", "MCLK", "MB4", "MB5"].contains(&k.as_str()),
                                    "mmv" => k.starts_with("MOVE_"),
                                    "msc" => k.starts_with("SCRL_"),
                                    "bt" => k.starts_with("BT_"),
                                    "rgb_ug" => k.starts_with("RGB_"),
                                    "bl" => k.starts_with("BL_"),
                                    "out" => k.starts_with("OUT_"),
                                    "ext_power" => k.starts_with("EP_"),
                                    _ => k.starts_with("BT_") || k.starts_with("RGB_") || k.starts_with("OUT_") || k.starts_with("MOVE_") || k.starts_with("SCRL_") || ["LCLK", "RCLK", "MCLK", "MB4", "MB5"].contains(&k.as_str())
                                }).map(|(k, v)| {
                                    let disp = if k != v { format!("{} ({})", k, v) } else { k.clone() };
                                    Suggestion { value: k, display: disp }
                                }).filter(|s| s.value.to_uppercase().contains(&query) || s.display.to_uppercase().contains(&query)).collect();

                                if behavior_name == "bt" && p_idx == 1 {
                                    for i in 0..5 {
                                        let val = i.to_string();
                                        if val.contains(&query) {
                                            results.push(Suggestion { value: val.clone(), display: format!("Profile {}", val) });
                                        }
                                    }
                                }
                                results
                            }                            _ => Vec::new()
                        }
                    } else { Vec::new() }
                } else { Vec::new() }
            };
            results.sort_by(|a, b| a.display.cmp(&b.display)); results
        }
    };
    let suggestions = get_suggestions();
    let on_keydown = {
        let current_text = current_text.clone(); let update_from_text = update_from_text.clone(); let on_apply = on_apply.clone(); let on_close = props.on_close.clone();
        let suggestions = suggestions.clone(); let suggestion_index = suggestion_index.clone(); let show_suggestions = show_suggestions.clone(); let suggestion_container_ref = suggestion_container_ref.clone();
        Callback::from(move |e: KeyboardEvent| {
            match e.key().as_str() {
                "Enter" => { e.prevent_default(); if *show_suggestions && !suggestions.is_empty() {
                    let selected = &suggestions[*suggestion_index].value; let parts: Vec<&str> = (*current_text).split_whitespace().collect(); let has_trailing_space = (*current_text).ends_with(' ');
                    let mut new_text = String::new(); if has_trailing_space || parts.is_empty() { new_text = format!("{}{}", *current_text, selected); } else { for (i, p) in parts.iter().enumerate() { if i == parts.len() - 1 { new_text.push_str(selected); } else { new_text.push_str(p); new_text.push(' '); } } }
                    if !selected.contains(' ') { new_text.push(' '); }
                    update_from_text.emit(new_text); show_suggestions.set(false);
                } else { on_apply.emit(MouseEvent::new("click").unwrap()); } }
                "Escape" => { on_close.emit(MouseEvent::new("click").unwrap()); }
                "Tab" => if *show_suggestions && !suggestions.is_empty() { e.prevent_default(); let selected = &suggestions[*suggestion_index].value; let parts: Vec<&str> = (*current_text).split_whitespace().collect(); let has_trailing_space = (*current_text).ends_with(' ');
                    let mut new_text = String::new(); if has_trailing_space || parts.is_empty() { new_text = format!("{}{}", *current_text, selected); } else { for (i, p) in parts.iter().enumerate() { if i == parts.len() - 1 { new_text.push_str(selected); } else { new_text.push_str(p); new_text.push(' '); } } }
                    if !selected.contains(' ') { new_text.push(' '); }
                    update_from_text.emit(new_text); show_suggestions.set(false); }
                "ArrowDown" => if *show_suggestions && !suggestions.is_empty() { e.prevent_default(); let next_idx = (*suggestion_index + 1) % suggestions.len(); suggestion_index.set(next_idx); if let Some(container) = suggestion_container_ref.cast::<web_sys::Element>() { let items = container.get_elements_by_class_name("suggestion-item"); if let Some(item) = items.get_with_index(next_idx as u32) { let item_el: web_sys::Element = item.dyn_into().unwrap(); item_el.scroll_into_view_with_bool(false); } } }
                "ArrowUp" => if *show_suggestions && !suggestions.is_empty() { e.prevent_default(); let next_idx = if *suggestion_index == 0 { suggestions.len() - 1 } else { *suggestion_index - 1 }; suggestion_index.set(next_idx); if let Some(container) = suggestion_container_ref.cast::<web_sys::Element>() { let items = container.get_elements_by_class_name("suggestion-item"); if let Some(item) = items.get_with_index(next_idx as u32) { let item_el: web_sys::Element = item.dyn_into().unwrap(); item_el.scroll_into_view_with_bool(true); } } }
                _ => { show_suggestions.set(true); suggestion_index.set(0); }
            }
        })
    };
    let mut max_x = 0; for pk in &props.data.physical_layout { if pk.x.abs() > max_x { max_x = pk.x.abs(); } }
    let u_pos = if max_x > 20000 { 19050.0 } else if max_x > 500 { 1000.0 } else { 100.0 };
    let mini_scale = 10.0 / u_pos;
    let mut current_binding_full = (*current_behavior_label).clone(); for p in &*current_params { current_binding_full.push(' '); current_binding_full.push_str(p); }
    let preview_parts = get_binding_parts(&current_binding_full);
    let selected_pk = &props.data.physical_layout[props.selected_key.key_index];
    let preview_style = format!("transform: rotate({}deg);", selected_pk.rotation as f32 / 1000.0);
    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
            <div class="bg-[#1a202c] text-white rounded-lg shadow-2xl flex max-w-5xl w-full overflow-hidden border border-gray-700 h-[80vh]">
                <div class="flex-1 p-8 overflow-y-auto flex flex-col">
                    <div class="flex justify-center mb-8 relative h-32 w-full shrink-0"> <div class="relative"> { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| { let is_selected = i == props.selected_key.key_index; let x = (pk.x as f32 * mini_scale) as i32; let y = (pk.y as f32 * mini_scale) as i32; let w = (pk.width as f32 * mini_scale).max(4.0) as i32 - 1; let h = (pk.height as f32 * mini_scale).max(4.0) as i32 - 1; let style = format!("left: {}px; top: {}px; width: {}px; height: {}px; transform: rotate({}deg);", x, y, w, h, pk.rotation as f32 / 1000.0); let class = if is_selected { "bg-green-500" } else { "bg-gray-700" }; html! { <div class={classes!("absolute", "rounded-sm", class)} style={style} /> } })} </div>
                        <div class="flex items-center ml-24 space-x-8"> <span class="text-2xl text-gray-400">{"→"}</span> <div class="bg-gray-800 w-16 h-16 rounded-lg border border-gray-600 flex items-center justify-center relative font-mono shadow-inner" style={preview_style}> { if !preview_parts.top_left.is_empty() { html! { <span class="absolute top-1 left-1 text-[8px] text-gray-400 leading-none">{preview_parts.top_left}</span> } } else { html! {} } } { if !preview_parts.top_right.is_empty() { html! { <span class="absolute top-1 right-1 text-[8px] text-gray-400 leading-none text-right max-w-[70%] truncate">{preview_parts.top_right}</span> } } else { html! {} } } <span class="text-xl font-bold">{preview_parts.center}</span> </div> </div>
                    </div>
                    <div class="mb-6 shrink-0"> <input ref={input_ref} type="text" class={classes!("w-full", "bg-gray-900", "border", "text-2xl", "p-4", "rounded", "font-mono", "focus:outline-none", if is_valid { vec!["border-gray-600", "focus:border-blue-500"] } else { vec!["border-red-500", "focus:border-red-400", "text-red-200"] })} value={(*current_text).clone()} oninput={let update = update_from_text.clone(); Callback::from(move |e: InputEvent| { let input: HtmlInputElement = e.target_unchecked_into(); update.emit(input.value()); })} onkeydown={on_keydown} /> { if !is_valid { html! { <div class="text-red-400 text-sm mt-1">{"Invalid binding: incomplete or incorrect parameters."}</div> } } else { html! {} }} </div>
                    <div class="border-t border-gray-700 my-6 shrink-0"></div>
                    <div class="mb-6 shrink-0"> <div class="flex items-center space-x-4"> <span class="text-xl font-semibold w-24">{"Behavior"}</span> <div onclick={let show = show_behavior_selection.clone(); Callback::from(move |_| show.set(!*show))} class="flex items-center space-x-2 bg-gray-800 px-3 py-1 rounded border border-gray-600 border-dashed cursor-pointer hover:bg-gray-700"> <span class="text-gray-300 font-mono">{&*current_behavior_label}</span> <span class="text-gray-500">{"|"}</span> <span class="text-gray-300">{display_name}</span> </div> </div>
                        { if *show_behavior_selection { html! { <div class="mt-4 grid grid-cols-3 gap-2 max-h-48 overflow-y-auto bg-black p-2 rounded border border-gray-700"> { for ZMK_BEHAVIORS.iter().map(|b| { let b_c = b; let onclick = { let select = select_behavior.clone(); Callback::from(move |_| select.emit(b_c)) }; html! { <div onclick={onclick} class="p-2 text-xs hover:bg-gray-800 cursor-pointer rounded border border-gray-800"> <div class="font-mono text-blue-400">{"&"}{b.label.unwrap_or(b.name)}</div> <div class="text-gray-500 truncate">{b.display_name.unwrap_or("")}</div> </div> } })} </div> } } else { html! {} }}
                    </div>
                    <div class="mb-10 grow overflow-y-auto"> <div class="text-xl font-semibold mb-4">{"Parameters"}</div> { if let Some(meta) = behavior_meta { html! { <div class="flex flex-col space-y-4 ml-8"> { for (0..expected_p_count).map(|i| { let ptype = meta.parameter_metadata.get(i).cloned().unwrap_or(ParameterType::Constant); let value = current_params.get(i).cloned().unwrap_or("UNKNOWN".to_string()); let label = match ptype { ParameterType::Layer => "Layer", ParameterType::Keycode => "Keycode", ParameterType::Modifier => "Modifier", ParameterType::Constant => "Constant", ParameterType::None => "None", };
                                    let display_value = match ptype { ParameterType::Layer => if let Ok(idx) = value.parse::<usize>() { props.data.layers.get(idx).map(|l| l.name.as_str()).unwrap_or(&value) } else { props.data.layers.iter().find(|l| l.name == value).map(|l| l.name.as_str()).unwrap_or(&value) }.to_string(), ParameterType::Keycode | ParameterType::Constant | ParameterType::Modifier => format_keycode(&value), _ => value.to_string(), };
                                    let onclick = { let on_toggle_param_selection = props.on_toggle_param_selection.clone(); let selected_param_idx = selected_param_idx.clone(); let show_param_selection = props.show_param_selection; Callback::from(move |e: MouseEvent| { if !show_param_selection || *selected_param_idx != i { selected_param_idx.set(i); if !show_param_selection { on_toggle_param_selection.emit(e); } } else { on_toggle_param_selection.emit(e); } }) };
                                    html! { <div class="flex items-center space-x-4"> <span class="text-gray-400 w-16">{label}</span> <div onclick={onclick} class={classes!("flex", "items-center", "space-x-2", "px-3", "py-1", "rounded", "border", "cursor-pointer", if props.show_param_selection && *selected_param_idx == i { vec!["bg-green-600", "border-green-400"] } else if value == "UNKNOWN" { vec!["bg-red-900", "border-red-500", "border-dashed"] } else { vec!["bg-gray-800", "border-gray-600", "border-dashed"] })}> <span class="font-mono">{value}</span> <span class="text-gray-500">{"|"}</span> <span class="">{display_value}</span> </div> </div> }
                                })} </div> } } else { html! { <div class="ml-8 text-gray-500 italic">{"No metadata for this behavior."}</div> } }}
                    </div>
                    <div class="flex justify-between space-x-4 mt-auto shrink-0 pt-4"> <button onclick={props.on_close.clone()} class="bg-gray-700 hover:bg-gray-600 text-white px-8 py-2 rounded font-semibold transition-colors"> {"Cancel"} </button> <button disabled={!is_valid} onclick={on_apply} class={classes!("px-8", "py-2", "rounded", "font-semibold", "transition-colors", if is_valid { vec!["bg-green-600", "hover:bg-green-700", "text-white"] } else { vec!["bg-gray-800", "text-gray-500", "cursor-not-allowed"] })}> {"Apply"} </button> </div>
                </div>
                <div class="w-80 bg-black border-l border-gray-700 flex flex-col h-full"> { if *show_suggestions && !suggestions.is_empty() { let text_val = (*current_text).clone(); let update = update_from_text.clone(); let show_sug = show_suggestions.clone();
                        html! { <div class="flex-1 flex flex-col overflow-hidden"> <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest shrink-0">{"Suggestions"}</div> <div class="flex-1 overflow-y-auto" ref={suggestion_container_ref}> { for suggestions.iter().enumerate().map(|(i, s)| { let is_active = i == *suggestion_index; let val = s.value.clone(); let text_val = text_val.clone(); let update = update.clone(); let show_sug = show_sug.clone(); let onclick = Callback::from(move |_| { let parts: Vec<&str> = text_val.split_whitespace().collect(); let has_trailing_space = text_val.ends_with(' '); let mut new_text = String::new(); if has_trailing_space || parts.is_empty() { new_text = format!("{}{}", text_val, val); } else { for (j, p) in parts.iter().enumerate() { if j == parts.len() - 1 { new_text.push_str(&val); } else { new_text.push_str(p); new_text.push(' '); } } }
                                        if !val.contains(' ') { new_text.push(' '); }
                                        update.emit(new_text); show_sug.set(false); });
                                        html! { <div onclick={onclick} class={classes!("suggestion-item", "p-3", "border-b", "border-gray-900", "cursor-pointer", "hover:bg-gray-900", "transition-colors", "font-mono", "text-sm", if is_active { vec!["bg-blue-900", "text-white", "border-blue-700"] } else { vec!["text-gray-300"] })}> {&s.display} </div> } })} </div> </div> }
                    } else if props.show_param_selection { let p_idx = *selected_param_idx; let p_type = behavior_meta.and_then(|m| m.parameter_metadata.get(p_idx)).cloned().unwrap_or(ParameterType::None);
                        match p_type {
                            ParameterType::Layer => html! { <div class="flex-1 flex flex-col h-full"> <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest">{"Select Layer"}</div> <div class="flex-1 overflow-y-auto"> { for props.data.layers.iter().enumerate().map(|(i, l)| { let is_active = current_params.get(p_idx).map(|p| *p == i.to_string()).unwrap_or(false); let val = i.to_string(); let select = select_param_value.clone(); let onclick = Callback::from(move |_| select.emit(val.clone()));
                                            html! { <div onclick={onclick} class={classes!("p-4", "border-b", "border-gray-800", "cursor-pointer", "hover:bg-gray-900", "transition-colors", if is_active { "bg-white text-black" } else { "" })}> <div class="font-bold">{i}</div> <div class={if is_active { "text-gray-600 italic" } else { "text-gray-400 italic" }}>{&l.name}</div> </div> } })} </div> <div class="p-2 flex justify-center border-t border-gray-700"> <button onclick={props.on_toggle_param_selection.clone()} class="text-xs text-gray-400 hover:text-white uppercase tracking-widest py-1 flex items-center"> <span class="rotate-90 inline-block mr-1">{"Close"}</span> </button> </div> </div> },
                            ParameterType::Keycode => { let filter_val = (*filter).clone();
                                let behavior_name = behavior_meta.map(|m| m.label.unwrap_or(m.name)).unwrap_or("");
                                let only_mods = is_modifier_only_param(behavior_name, p_idx);
                                let is_hold_tap = behavior_meta.map(|m| m.compatible == Some("zmk,behavior-hold-tap")).unwrap_or(false);
                                let only_regular = behavior_name == "kp" || behavior_name == "kt" || is_hold_tap;
                                let is_tap_param = is_hold_tap && p_idx == 1;
                                html! { <div class="flex-1 flex flex-col h-full"> <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest"> {if only_mods { "Select Modifier" } else { "Select Keycode" }} </div> <div class="p-2 border-b border-gray-700"> <input type="text" placeholder="Search..." class="w-full bg-gray-900 text-white text-xs p-1 rounded focus:outline-none focus:ring-1 focus:ring-blue-500" oninput={let filter = filter.clone(); Callback::from(move |e: InputEvent| { let input: HtmlInputElement = e.target_unchecked_into(); filter.set(input.value().to_uppercase()); })} value={filter_val.clone()} /> </div> <div class="flex-1 overflow-y-auto"> { for keycodes::KEY_ALIASES.iter().filter(|(&k, _)| !only_mods || keycodes::is_modifier(k)).filter(|(&k, _)| {
                                    if is_tap_param {
                                        keycodes::is_plain_key(k)
                                    } else if only_regular {
                                        keycodes::is_regular_key(k)
                                    } else {
                                        true
                                    }
                                }).filter(|(&k, &v)| k.to_uppercase().contains(&filter_val) || v.to_uppercase().contains(&filter_val)).map(|(&k, &v)| {
                                                let val = k.to_string(); let select = select_param_value.clone(); let is_active = current_params.get(p_idx).map(|p| *p == val).unwrap_or(false); let val_c = val.clone(); let onclick = Callback::from(move |_| select.emit(val_c.clone()));
                                                html! { <div onclick={onclick} class={classes!("p-2", "border-b", "border-gray-800", "cursor-pointer", "hover:bg-gray-900", "transition-colors", "text-xs", if is_active { "bg-white text-black" } else { "" })}> <div class="font-bold font-mono">{val}</div> <div class={if is_active { "text-gray-600" } else { "text-gray-400" }}>{v}</div> </div> } })} </div> <div class="p-2 flex justify-center border-t border-gray-700"> <button onclick={props.on_toggle_param_selection.clone()} class="text-xs text-gray-400 hover:text-white uppercase tracking-widest py-1 flex items-center"> <span class="rotate-90 inline-block mr-1">{"Close"}</span> </button> </div> </div> } },
                            ParameterType::Constant => { let filter_val = (*filter).clone();
                                let behavior_name = behavior_meta.map(|m| m.label.unwrap_or(m.name)).unwrap_or("");
                                let mut constants_list: Vec<(String, String)> = Vec::new();
                                if let Some(meta) = behavior_meta {
                                    if !meta.constants.is_empty() {
                                        constants_list = meta.constants.iter().map(|&k| (k.to_string(), k.to_string())).collect();
                                    }
                                }
                                if constants_list.is_empty() {
                                    constants_list = keycodes::KEY_ALIASES.iter().map(|(&k, &v)| (k.to_string(), v.to_string())).collect();
                                }

                                constants_list = constants_list.into_iter().filter(|(k, _)| match behavior_name {
                                    "mkp" => ["LCLK", "RCLK", "MCLK", "MB4", "MB5"].contains(&k.as_str()),
                                    "mmv" => k.starts_with("MOVE_"),
                                    "msc" => k.starts_with("SCRL_"),
                                    "bt" => k.starts_with("BT_"),
                                    "rgb_ug" => k.starts_with("RGB_"),
                                    "bl" => k.starts_with("BL_"),
                                    "out" => k.starts_with("OUT_"),
                                    "ext_power" => k.starts_with("EP_"),
                                    _ => k.starts_with("BT_") || k.starts_with("RGB_") || k.starts_with("OUT_") || k.starts_with("MOVE_") || k.starts_with("SCRL_") || ["LCLK", "RCLK", "MCLK", "MB4", "MB5"].contains(&k.as_str())
                                }).collect();

                                if behavior_name == "bt" && p_idx == 1 {
                                    for i in 0..5 { constants_list.push((i.to_string(), format!("Profile {}", i))); }
                                }

                                html! { <div class="flex-1 flex flex-col h-full"> <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest"> {"Select Constant"} </div> <div class="p-2 border-b border-gray-700"> <input type="text" placeholder="Search..." class="w-full bg-gray-900 text-white text-xs p-1 rounded focus:outline-none focus:ring-1 focus:ring-blue-500" oninput={let filter = filter.clone(); Callback::from(move |e: InputEvent| { let input: HtmlInputElement = e.target_unchecked_into(); filter.set(input.value().to_uppercase()); })} value={filter_val.clone()} /> </div> <div class="flex-1 overflow-y-auto"> { for constants_list.iter().filter(|(k, v)| k.to_uppercase().contains(&filter_val) || v.to_uppercase().contains(&filter_val)).map(|(k, v)| {
                                                let val = k.to_string(); let select = select_param_value.clone(); let is_active = current_params.get(p_idx).map(|p| *p == val).unwrap_or(false); let val_c = val.clone(); let onclick = Callback::from(move |_| select.emit(val_c.clone()));
                                                html! { <div onclick={onclick} class={classes!("p-2", "border-b", "border-gray-800", "cursor-pointer", "hover:bg-gray-900", "transition-colors", "text-xs", if is_active { "bg-white text-black" } else { "" })}> <div class="font-bold font-mono">{val}</div> <div class={if is_active { "text-gray-600" } else { "text-gray-400" }}>{v}</div> </div> } })} </div> <div class="p-2 flex justify-center border-t border-gray-700"> <button onclick={props.on_toggle_param_selection.clone()} class="text-xs text-gray-400 hover:text-white uppercase tracking-widest py-1 flex items-center"> <span class="rotate-90 inline-block mr-1">{"Close"}</span> </button> </div> </div> }
                            },
                            _ => html! { <div class="flex-1 flex flex-col items-center justify-center p-4 text-center"> <div class="text-gray-500 italic">{"Selection not implemented for this parameter type."}</div> <button onclick={props.on_toggle_param_selection.clone()} class="mt-4 text-xs text-gray-400 hover:text-white uppercase tracking-widest"> {"Close"} </button> </div> }
                        }
                    } else { html! { <div class="flex-1 flex flex-col items-center justify-center p-8 text-center text-gray-600"> <div class="text-4xl mb-4">{"⌨️"}</div> <div class="text-sm">{"Type to see suggestions or click a parameter to select from list."}</div> </div> } }
                    } </div> </div> </div> }
}
