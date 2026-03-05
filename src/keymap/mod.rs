use yew::prelude::*;
use web_sys::{HtmlInputElement, FileReader};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;

pub mod behaviors;
use behaviors::{ZMK_BEHAVIORS, ParameterType};

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
        "&mt" => BindingParts {
            top_left: "".into(),
            top_right: format!("mt {}", params.get(0).map(|&p| format_keycode(p)).unwrap_or_else(|| "".to_string())),
            center: params.get(1).map(|&p| format_keycode(p)).unwrap_or_else(|| "".to_string()),
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
    let error = use_state(|| None::<String>);
    let loading = use_state(|| false);

    let on_file_input = {
        let keymap_data = keymap_data.clone();
        let error = error.clone();
        let loading = loading.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    let reader = FileReader::new().unwrap();
                    let reader_c = reader.clone();
                    let keymap_data = keymap_data.clone();
                    let error = error.clone();
                    let loading = loading.clone();

                    let onload = Closure::wrap(Box::new(move |_e: ProgressEvent| {
                        let content = reader_c.result().unwrap().as_string().unwrap();
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

    html! {
        <div class="w-full flex flex-col items-center p-4">
            <h2 class="text-4xl font-display mb-8">{"ZMK Keymap Editor"}</h2>
            
            <div class="mb-8">
                <label class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Upload .keymap file"}</label>
                <input 
                    type="file" 
                    oninput={on_file_input}
                    class="block w-full text-sm text-gray-900 border border-gray-300 rounded-lg cursor-pointer bg-gray-50 dark:text-gray-400 focus:outline-none dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400"
                />
            </div>

            { if *loading {
                html! { <div class="text-blue-500 mb-4 animate-pulse">{"Parsing keymap on server..."}</div> }
            } else {
                html! {}
            }}

            { if let Some(err) = &*error {
                html! { <div class="text-red-500 mb-4">{err}</div> }
            } else {
                html! {}
            }}

            { if let Some(data) = &*keymap_data {
                html! { <KeymapRenderer data={data.clone()} /> }
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
}

#[function_component]
fn KeymapRenderer(props: &RendererProps) -> Html {
    let current_layer = use_state(|| 0);
    let selected_key = use_state(|| None::<SelectedKey>);
    let show_param_selection = use_state(|| false);

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

    let close_popup = {
        let selected_key = selected_key.clone();
        let show_param_selection = show_param_selection.clone();
        Callback::from(move |_: MouseEvent| {
            selected_key.set(None);
            show_param_selection.set(false);
        })
    };

    let toggle_param_selection = {
        let show_param_selection = show_param_selection.clone();
        Callback::from(move |_: MouseEvent| {
            show_param_selection.set(!*show_param_selection);
        })
    };

    html! {
        <div class="flex flex-col items-center w-full mt-4">
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

                    let style = format!("left: {}px; top: {}px; width: {}px; height: {}px;", x, y, w, h);
                    
                    let onclick = {
                        let on_key_click = on_key_click.clone();
                        Callback::from(move |_| on_key_click.emit(i))
                    };

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
}

#[function_component]
fn KeyBindingPopup(props: &PopupProps) -> Html {
    let binding = &props.data.layers[props.selected_key.layer_index].bindings[props.selected_key.key_index];
    
    // Split binding into behavior and parameters
    let parts: Vec<&str> = binding.split_whitespace().collect();
    let behavior_label = parts.get(0).cloned().unwrap_or("");
    let params = parts[1..].to_vec();

    // Find behavior in metadata
    let behavior_name = behavior_label.strip_prefix('&').unwrap_or(behavior_label);
    let behavior_meta = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name);

    let display_name = behavior_meta.and_then(|m| m.display_name).unwrap_or(behavior_name);

    let selected_param_idx = use_state(|| 0usize);

    // Mini-map scaling
    let mut max_x = 0;
    for pk in &props.data.physical_layout {
        if pk.x.abs() > max_x { max_x = pk.x.abs(); }
    }
    let u_pos = if max_x > 20000 { 19050.0 } else if max_x > 500 { 1000.0 } else { 100.0 };
    let mini_scale = 10.0 / u_pos;

    let preview_parts = get_binding_parts(binding);
    let tl = if !preview_parts.top_left.is_empty() {
        html! { <span class="absolute top-1 left-1 text-[8px] text-gray-400 leading-none">{&preview_parts.top_left}</span> }
    } else {
        html! {}
    };
    let tr = if !preview_parts.top_right.is_empty() {
        html! { <span class="absolute top-1 right-1 text-[8px] text-gray-400 leading-none text-right max-w-[70%] truncate">{&preview_parts.top_right}</span> }
    } else {
        html! {}
    };

    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
            <div class="bg-[#1a202c] text-white rounded-lg shadow-2xl flex max-w-4xl w-full overflow-hidden border border-gray-700">
                <div class="flex-1 p-8">
                    // Keyboard mini-map
                    <div class="flex justify-center mb-8 relative h-32 w-full">
                        <div class="relative">
                            { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| {
                                let is_selected = i == props.selected_key.key_index;
                                let x = (pk.x as f32 * mini_scale) as i32;
                                let y = (pk.y as f32 * mini_scale) as i32;
                                let w = (pk.width as f32 * mini_scale).max(4.0) as i32 - 1;
                                let h = (pk.height as f32 * mini_scale).max(4.0) as i32 - 1;
                                let style = format!("left: {}px; top: {}px; width: {}px; height: {}px;", x, y, w, h);
                                let class = if is_selected { "bg-green-500" } else { "bg-gray-700" };
                                html! { <div class={classes!("absolute", "rounded-sm", class)} style={style} /> }
                            })}
                        </div>
                        
                        // Arrow and current binding preview
                        <div class="flex items-center ml-24 space-x-8">
                            <span class="text-2xl text-gray-400">{"→"}</span>
                            <div class="bg-gray-800 w-16 h-16 rounded-lg border border-gray-600 flex items-center justify-center relative font-mono shadow-inner">
                                {tl}
                                {tr}
                                <span class="text-xl font-bold">{&preview_parts.center}</span>
                            </div>
                        </div>
                    </div>

                    <div class="border-t border-gray-700 my-6"></div>

                    // Behavior Selection
                    <div class="mb-6">
                        <div class="flex items-center space-x-4">
                            <span class="text-xl font-semibold w-24">{"Behavior"}</span>
                            <div class="flex items-center space-x-2 bg-gray-800 px-3 py-1 rounded border border-gray-600 border-dashed">
                                <span class="text-gray-300 font-mono">{behavior_label}</span>
                                <span class="text-gray-500">{"|"}</span>
                                <span class="text-gray-300">{display_name}</span>
                            </div>
                        </div>
                    </div>

                    // Parameters
                    <div class="mb-10">
                        <div class="text-xl font-semibold mb-4">{"Parameters"}</div>
                        { if let Some(meta) = behavior_meta {
                            html! {
                                <div class="flex flex-col space-y-4 ml-8">
                                { for meta.parameter_metadata.iter().enumerate().map(|(i, ptype)| {
                                    let value = params.get(i).cloned().unwrap_or("");
                                    let label = match ptype {
                                        ParameterType::Layer => "Layer",
                                        ParameterType::Keycode => "Keycode",
                                        ParameterType::None => "None",
                                    };
                                    
                                    let display_value = match ptype {
                                        ParameterType::Layer => {
                                            if let Ok(idx) = value.parse::<usize>() {
                                                props.data.layers.get(idx).map(|l| l.name.as_str()).unwrap_or("unknown")
                                            } else { "unknown" }.to_string()
                                        }
                                        ParameterType::Keycode => format_keycode(&value),
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
                                                    if is_active { "bg-green-600 border-green-400" } else { "bg-gray-800 border-gray-600 border-dashed" }
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
                    <div class="flex justify-center space-x-4">
                        <button class="bg-green-600 hover:bg-green-700 text-white px-8 py-2 rounded font-semibold transition-colors">
                            {"Apply"}
                        </button>
                        <button onclick={props.on_close.clone()} class="bg-gray-700 hover:bg-gray-600 text-white px-8 py-2 rounded font-semibold transition-colors">
                            {"Cancel"}
                        </button>
                    </div>
                </div>

                // Side Panel for Parameter Selection
                { if props.show_param_selection {
                    let p_idx = *selected_param_idx;
                    let p_type = behavior_meta.and_then(|m| m.parameter_metadata.get(p_idx)).cloned().unwrap_or(ParameterType::None);
                    
                    match p_type {
                        ParameterType::Layer => html! {
                            <div class="w-64 bg-black border-l border-gray-700 flex flex-col">
                                <div class="flex-1 overflow-y-auto">
                                    { for props.data.layers.iter().enumerate().map(|(i, l)| {
                                        let is_active = params.get(p_idx).map(|p| *p == i.to_string()).unwrap_or(false);
                                        html! {
                                            <div class={classes!(
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
                        _ => html! {
                            <div class="w-64 bg-black border-l border-gray-700 flex flex-col items-center justify-center p-4 text-center">
                                <div class="text-gray-500 italic">{"Selection not implemented for this parameter type."}</div>
                                <button onclick={props.on_toggle_param_selection.clone()} class="mt-4 text-xs text-gray-400 hover:text-white uppercase tracking-widest">
                                    {"Close"}
                                </button>
                            </div>
                        }
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}
