use gloo_net::http::Request;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use serde::{Deserialize, Serialize};

use crate::keymap::{
    show_open_file_picker, BindingParts, FileSystemFileHandle, FileSystemWritableFileStream,
    KeymapData, SelectedKey,
};

pub mod layout;

fn is_mac() -> bool {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        let platform = navigator.platform().unwrap_or_default().to_lowercase();
        platform.contains("mac") || platform.contains("iphone") || platform.contains("ipad") || platform.contains("ipod")
    } else {
        false
    }
}

fn format_kanata_keycode(kc: &str, is_mac: bool) -> String {
    match kc {
        "ent" | "enter" | "ret" => "⏎".to_string(),
        "bspc" | "backspace" => "⌫".to_string(),
        "spc" | "space" => "SPACE".to_string(),
        "tab" => "⇥".to_string(),
        "esc" | "escape" => "ESC".to_string(),
        "lsft" => "⇧".to_string(),
        "rsft" => "⇧".to_string(),
        "lctl" => if is_mac { "⌃".to_string() } else { "CTRL".to_string() },
        "rctl" => if is_mac { "⌃".to_string() } else { "CTRL".to_string() },
        "lalt" => if is_mac { "⌥".to_string() } else { "ALT".to_string() },
        "ralt" => if is_mac { "⌥".to_string() } else { "ALT".to_string() },
        "lmet" => if is_mac { "⌘".to_string() } else { "⊞".to_string() },
        "rmet" => if is_mac { "⌘".to_string() } else { "⊞".to_string() },
        "caps" | "capslock" => "⇪".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "pgup" => "⇞".to_string(),
        "pgdn" => "⇟".to_string(),
        "home" => "↖".to_string(),
        "end" => "↘".to_string(),
        "ins" => "INS".to_string(),
        "del" => "⌦".to_string(),
        "mlft" => "🖱️1".to_string(),
        "mrgt" => "🖱️2".to_string(),
        "mmid" => "🖱️3".to_string(),
        "mbck" => "🖱️4".to_string(),
        "mfwd" => "🖱️5".to_string(),
        "mup" => "🖱️↑".to_string(),
        "mdown" => "🖱️↓".to_string(),
        "mleft" => "🖱️←".to_string(),
        "mright" => "🖱️→".to_string(),
        "mwl" => "🖱️←".to_string(),
        "mwr" => "🖱️→".to_string(),
        "mwu" => "🖱️↑".to_string(),
        "mwd" => "🖱️↓".to_string(),
        "comm" => ",".to_string(),
        "dot" => ".".to_string(),
        "slsh" => "/".to_string(),
        "scln" => ";".to_string(),
        "apos" => "'".to_string(),
        "lbkt" => "[".to_string(),
        "rbkt" => "]".to_string(),
        "bksl" => "\\".to_string(),
        "grv" => "`".to_string(),
        "min" => "-".to_string(),
        "eql" => "=".to_string(),
        s if s.starts_with('f') && s[1..].parse::<u32>().is_ok() => s.to_uppercase(),
        s => s.to_uppercase(),
    }
}

fn get_kanata_binding_parts(binding: &str, aliases: &std::collections::HashMap<String, String>, is_mac: bool, _is_laptop: bool) -> BindingParts {
    let mut current = binding.to_string();
    let mut top_left = "".to_string();
    let mut top_right = "".to_string();
    let mut center = "".to_string();

    let lookup_key = binding.strip_prefix('@').unwrap_or(binding);
    if let Some(aliased) = aliases.get(lookup_key) {
        current = aliased.clone();
    }

    if current.starts_with('(') && current.ends_with(')') {
        let inner = &current[1..current.len() - 1];
        let parts: Vec<&str> = inner.split_whitespace().collect();
        if parts.is_empty() {
            return BindingParts { top_left, top_right, center };
        }
        match parts[0] {
            "tap-hold" | "tap-hold-press" | "tap-hold-release" => {
                top_left = "MT".to_string();
                if parts.len() >= 5 {
                    center = format_kanata_keycode(parts[3], is_mac);
                    top_right = format_kanata_keycode(parts[4], is_mac);
                }
            }
            "layer-toggle" | "layer-switch" | "layer-while-held" => {
                top_left = match parts[0] {
                    "layer-toggle" => "LT".to_string(),
                    "layer-switch" => "LS".to_string(),
                    _ => "LH".to_string(),
                };
                if parts.len() >= 2 {
                    center = parts[1].to_uppercase();
                }
            }
            "one-shot" => {
                top_left = "OS".to_string();
                if parts.len() >= 3 {
                    center = format_kanata_keycode(parts[2], is_mac);
                }
            }
            "multi" => {
                top_left = "M".to_string();
                center = parts.get(1).map(|&s| format_kanata_keycode(s, is_mac)).unwrap_or_else(|| "...".to_string());
            }
            "macro" => {
                top_left = "MC".to_string();
                center = parts.get(1).map(|&s| format_kanata_keycode(s, is_mac)).unwrap_or_else(|| "...".to_string());
            }
            _ => {
                top_left = parts[0].to_uppercase();
                center = parts.get(1).map(|&s| format_kanata_keycode(s, is_mac)).unwrap_or_default();
            }
        }
    } else {
        match current.as_str() {
            "1" => { center = "1".to_string(); top_right = "!".to_string(); }
            "2" => { center = "2".to_string(); top_right = "@".to_string(); }
            "3" => { center = "3".to_string(); top_right = "#".to_string(); }
            "4" => { center = "4".to_string(); top_right = "$".to_string(); }
            "5" => { center = "5".to_string(); top_right = "%".to_string(); }
            "6" => { center = "6".to_string(); top_right = "^".to_string(); }
            "7" => { center = "7".to_string(); top_right = "&".to_string(); }
            "8" => { center = "8".to_string(); top_right = "*".to_string(); }
            "9" => { center = "9".to_string(); top_right = "(".to_string(); }
            "0" => { center = "0".to_string(); top_right = ")".to_string(); }
            "-" | "min" => { center = "-".to_string(); top_right = "_".to_string(); }
            "=" | "eql" => { center = "=".to_string(); top_right = "+".to_string(); }
            "[" | "lbkt" => { center = "[".to_string(); top_right = "{".to_string(); }
            "]" | "rbkt" => { center = "]".to_string(); top_right = "}".to_string(); }
            "\\" | "bksl" => { center = "\\".to_string(); top_right = "|".to_string(); }
            ";" | "scln" => { center = ";".to_string(); top_right = ":".to_string(); }
            "'" | "apos" => { center = "'".to_string(); top_right = "\"".to_string(); }
            "," | "comm" => { center = ",".to_string(); top_right = "<".to_string(); }
            "." | "dot" => { center = ".".to_string(); top_right = ">".to_string(); }
            "/" | "slsh" => { center = "/".to_string(); top_right = "?".to_string(); }
            "grv" => { center = "`".to_string(); top_right = "~".to_string(); }
            _ => { center = format_kanata_keycode(&current, is_mac); }
        }
    }

    BindingParts { top_left, top_right, center }
}

struct KanataActionInfo {
    name: &'static str,
    params: &'static [ParamType],
    description: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
enum ParamType {
    Timeout,
    Action,
    Layer,
    Any,
}

static KANATA_ACTIONS: &[KanataActionInfo] = &[
    KanataActionInfo {
        name: "tap-hold",
        params: &[ParamType::Timeout, ParamType::Timeout, ParamType::Action, ParamType::Action],
        description: "Tap for one action, hold for another.",
    },
    KanataActionInfo {
        name: "tap-hold-press",
        params: &[ParamType::Timeout, ParamType::Timeout, ParamType::Action, ParamType::Action],
        description: "Similar to tap-hold, but hold action triggers on press.",
    },
    KanataActionInfo {
        name: "tap-hold-release",
        params: &[ParamType::Timeout, ParamType::Timeout, ParamType::Action, ParamType::Action],
        description: "Similar to tap-hold, but hold action triggers on release.",
    },
    KanataActionInfo {
        name: "tap-hold-next",
        params: &[ParamType::Timeout, ParamType::Action, ParamType::Action],
        description: "Tap for one action, hold for another. Hold triggers if another key is pressed.",
    },
    KanataActionInfo {
        name: "tap-hold-next-release",
        params: &[ParamType::Timeout, ParamType::Action, ParamType::Action],
        description: "Similar to tap-hold-next, but triggers on release of the other key.",
    },
    KanataActionInfo {
        name: "layer-toggle",
        params: &[ParamType::Layer],
        description: "Switch to layer while held.",
    },
    KanataActionInfo {
        name: "layer-switch",
        params: &[ParamType::Layer],
        description: "Switch to layer permanently.",
    },
    KanataActionInfo {
        name: "layer-while-held",
        params: &[ParamType::Layer],
        description: "Switch to layer while held (alias for layer-toggle).",
    },
    KanataActionInfo {
        name: "macro",
        params: &[ParamType::Any],
        description: "Run a sequence of actions.",
    },
    KanataActionInfo {
        name: "multi",
        params: &[ParamType::Any],
        description: "Run multiple actions simultaneously.",
    },
    KanataActionInfo {
        name: "one-shot",
        params: &[ParamType::Timeout, ParamType::Action],
        description: "Action stays active for timeout or until next press.",
    },
    KanataActionInfo {
        name: "tap-dance",
        params: &[ParamType::Timeout, ParamType::Any],
        description: "Different actions based on number of taps: (tap-dance timeout (action1 action2 ...))",
    },
    KanataActionInfo {
        name: "caps-word",
        params: &[ParamType::Timeout],
        description: "Capitalize the next word.",
    },
    KanataActionInfo {
        name: "unicode",
        params: &[ParamType::Any],
        description: "Send a unicode character.",
    },
];

struct KanataValidator<'a> {
    data: &'a KeymapData,
}

impl<'a> KanataValidator<'a> {
    fn new(data: &'a KeymapData) -> Self {
        Self { data }
    }

    fn validate_action(&self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() { return false; }
        if text == "_" || text == "XX" { return true; }
        
        if text.starts_with('@') {
            return self.data.aliases.contains_key(&text[1..]);
        }

        if text.starts_with('(') && text.ends_with(')') {
            let inner = &text[1..text.len()-1];
            let parts = self.split_parts(inner);
            if parts.is_empty() { return false; }
            
            if let Some(action) = KANATA_ACTIONS.iter().find(|a| a.name == parts[0]) {
                let params = &parts[1..];
                
                // Variadic or special list actions
                if action.name == "multi" || action.name == "macro" {
                    return !params.is_empty();
                }

                if action.name == "tap-dance" {
                    if params.len() != 2 { return false; }
                    if params[0].parse::<u32>().is_err() { return false; }
                    let list = params[1];
                    if !list.starts_with('(') || !list.ends_with(')') { return false; }
                    let sub_actions = self.split_parts(&list[1..list.len()-1]);
                    return !sub_actions.is_empty() && sub_actions.iter().all(|&a| self.validate_action(a));
                }

                if params.len() != action.params.len() { return false; }

                for (i, &p_type) in action.params.iter().enumerate() {
                    let val = params[i];
                    match p_type {
                        ParamType::Timeout => {
                            if val.parse::<u32>().is_err() { return false; }
                        }
                        ParamType::Layer => {
                            if !self.data.layers.iter().any(|l| l.name == val) { return false; }
                        }
                        ParamType::Action => {
                            if !self.validate_action(val) && !KANATA_KEYS.contains(&val) {
                                return false;
                            }
                        }
                        ParamType::Any => {}
                    }
                }
                return true;
            }
            return false;
        }

        KANATA_KEYS.contains(&text) || self.data.aliases.contains_key(text)
    }

    fn split_parts<'b>(&self, text: &'b str) -> Vec<&'b str> {
        let mut parts = Vec::new();
        let mut depth = 0;
        let mut start = 0;
        let bytes = text.as_bytes();
        
        for i in 0..bytes.len() {
            match bytes[i] as char {
                '(' => depth += 1,
                ')' => depth -= 1,
                ' ' | '\t' | '\n' if depth == 0 => {
                    let part = text[start..i].trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                    start = i + 1;
                }
                _ => {}
            }
        }
        let last = text[start..].trim();
        if !last.is_empty() {
            parts.push(last);
        }
        parts
    }

    fn validate_full(&self, text: &str) -> bool {
        let text = text.trim();
        if text.contains('=') {
            let (name, val) = text.split_once('=').unwrap();
            !name.trim().is_empty() && self.validate_action(val.trim())
        } else {
            self.validate_action(text)
        }
    }
}

static KANATA_KEYS: &[&str] = &[
    "lsft", "rsft", "lctl", "rctl", "lalt", "ralt", "lmet", "rmet",
    "caps", "esc", "ent", "bspc", "spc", "tab", "del", "ins",
    "up", "down", "left", "right", "pgup", "pgdn", "home", "end",
    "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "0",
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
    "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
    "comm", "dot", "slsh", "scln", "apos", "lbkt", "rbkt", "bksl", "grv", "min", "eql",
];

async fn is_laptop() -> bool {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        let get_battery_val = match js_sys::Reflect::get(&navigator, &"getBattery".into()) {
            Ok(val) => val,
            Err(_) => return false,
        };
        
        if get_battery_val.is_function() {
            let promise_val = match js_sys::Reflect::apply(&get_battery_val.unchecked_into(), &navigator, &js_sys::Array::new()) {
                Ok(val) => val,
                Err(_) => return false,
            };
            let promise: js_sys::Promise = promise_val.unchecked_into();
            let result = wasm_bindgen_futures::JsFuture::from(promise).await;
            return result.is_ok();
        }
    }
    false
}

#[derive(Serialize)]
struct KanataRequest {
    content: String,
    is_mac: bool,
    is_laptop: bool,
}

#[derive(Serialize)]
struct SaveKanataRequest {
    original_content: String,
    data: KeymapData,
}

#[derive(Deserialize)]
struct SaveKanataResponse {
    content: String,
}

#[function_component]
pub fn KanataHome() -> Html {
    let kanata_data = use_state(|| None::<KeymapData>);
    let original_content = use_state(|| String::new());
    let error = use_state(|| None::<String>);
    let loading = use_state(|| false);
    let file_handle = use_state(|| None::<FileSystemFileHandle>);
    let current_layer = use_state(|| 0);
    let selected_key = use_state(|| None::<SelectedKey>);
    let is_laptop_state = use_state(|| false);

    let on_open = {
        let kanata_data = kanata_data.clone();
        let original_content = original_content.clone();
        let error = error.clone();
        let loading = loading.clone();
        let file_handle = file_handle.clone();
        let is_laptop_state = is_laptop_state.clone();
        Callback::from(move |_| {
            let kanata_data = kanata_data.clone();
            let original_content = original_content.clone();
            let error = error.clone();
            let loading = loading.clone();
            let file_handle = file_handle.clone();
            let is_laptop_state = is_laptop_state.clone();
            spawn_local(async move {
                let options = js_sys::Object::new();
                let types = js_sys::Array::new();
                let type0 = js_sys::Object::new();
                js_sys::Reflect::set(&type0, &"description".into(), &"Kanata KBD Files".into()).unwrap();
                let accept = js_sys::Object::new();
                let extensions = js_sys::Array::new();
                extensions.push(&".kbd".into());
                js_sys::Reflect::set(&accept, &"text/plain".into(), &extensions).unwrap();
                js_sys::Reflect::set(&type0, &"accept".into(), &accept).unwrap();
                types.push(&type0);
                js_sys::Reflect::set(&options, &"types".into(), &types).unwrap();
                
                let picker_promise = show_open_file_picker(&options);
                let result = wasm_bindgen_futures::JsFuture::from(picker_promise).await;

                match result {
                    Ok(handles) => {
                        let handles: js_sys::Array = handles.unchecked_into();
                        if handles.length() > 0 {
                            let handle: FileSystemFileHandle = handles.get(0).unchecked_into();
                            file_handle.set(Some(handle.clone()));

                            loading.set(true);
                            let laptop = is_laptop().await;
                            is_laptop_state.set(laptop);
                            
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

                                            let parse_result = Request::post("/api/parse-kanata")
                                                .json(&KanataRequest { 
                                                    content, 
                                                    is_mac: is_mac(),
                                                    is_laptop: laptop
                                                })
                                                .unwrap()
                                                .send()
                                                .await;

                                            loading.set(false);
                                            match parse_result {
                                                Ok(resp) => {
                                                    if resp.ok() {
                                                        match resp.json::<KeymapData>().await {
                                                            Ok(data) => {
                                                                kanata_data.set(Some(data));
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
                                            error.set(Some(format!("Failed to read file: {:?}", e)));
                                        }
                                    }
                                }
                                Err(e) => {
                                    loading.set(false);
                                    error.set(Some(format!("Failed to get file: {:?}", e)));
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

    let on_save = {
        let kanata_data = kanata_data.clone();
        let original_content = original_content.clone();
        let error = error.clone();
        let loading = loading.clone();
        let file_handle = file_handle.clone();
        Callback::from(move |_| {
            if let Some(data) = &*kanata_data {
                let original_content_str = (*original_content).clone();
                let data = data.clone();
                let error = error.clone();
                let loading = loading.clone();
                let file_handle_val = (*file_handle).clone();

                loading.set(true);
                spawn_local(async move {
                    let result = Request::post("/api/save-kanata")
                        .json(&SaveKanataRequest {
                            original_content: original_content_str,
                            data,
                        })
                        .unwrap()
                        .send()
                        .await;

                    match result {
                        Ok(resp) => {
                            if resp.ok() {
                                match resp.json::<SaveKanataResponse>().await {
                                    Ok(res) => {
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
                                        }
                                    }
                                    Err(e) => {
                                        loading.set(false);
                                        error.set(Some(format!("Failed to parse response: {}", e)));
                                    }
                                }
                            } else {
                                loading.set(false);
                                error.set(Some(format!("Server error: {}", resp.text().await.unwrap_or_default())));
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

    html! {
        <div class="w-full flex flex-col items-center p-4">
            <h2 class="text-4xl font-display mb-8">{"Kanata Editor"}</h2>

            <div class="flex items-center space-x-4 mb-8">
                <div>
                    <div class="flex flex-col space-y-2">
                        <label class="block text-sm font-medium text-gray-900 dark:text-white">{"Open .kbd file"}</label>
                        <button onclick={on_open} class="px-6 py-2.5 bg-blue-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-blue-700 hover:shadow-lg focus:bg-blue-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-blue-800 active:shadow-lg transition duration-150 ease-in-out">
                            {"Open File"}
                        </button>
                    </div>
                    { if kanata_data.is_some() {
                        html! {
                            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                                {"Type "} <kbd class="px-1.5 py-0.5 font-sans font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">{"j"}</kbd> {" to start jump mode"}
                            </p>
                        }
                    } else { html! {} }}
                </div>
                { if kanata_data.is_some() {
                    html! {
                        <div class="flex space-x-2 mt-6">
                            <button onclick={on_save} class="px-6 py-2.5 bg-green-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-green-700 hover:shadow-lg focus:bg-green-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-green-800 active:shadow-lg transition duration-150 ease-in-out">
                                {"Save File"}
                            </button>
                        </div>
                    }
                } else { html! {} }}
            </div>

            { if *loading { html! { <div class="text-blue-500 mb-4 animate-pulse">{"Processing..."}</div> } } else { html! {} }}
            { if let Some(err) = &*error { html! { <div class="text-red-500 mb-4">{err}</div> } } else { html! {} }}

            { if let Some(data) = &*kanata_data {
                let on_update = {
                    let kanata_data = kanata_data.clone();
                    Callback::from(move |d| kanata_data.set(Some(d)))
                };
                html! { <KanataRenderer 
                    data={data.clone()} 
                    on_update={on_update} 
                    current_layer={current_layer}
                    selected_key={selected_key}
                    is_laptop={*is_laptop_state}
                /> }
            } else {
                html! { <div class="text-gray-500 italic">{"Please open a Kanata configuration file."}</div> }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct RendererProps {
    data: KeymapData,
    on_update: Callback<KeymapData>,
    current_layer: UseStateHandle<usize>,
    selected_key: UseStateHandle<Option<SelectedKey>>,
    is_laptop: bool,
}

#[function_component]
fn KanataRenderer(props: &RendererProps) -> Html {
    let jump_mode_active = use_state(|| false);
    let jump_input = use_state(|| String::new());
    let container_ref = use_node_ref();

    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let mut hint_map = std::collections::HashMap::new();
    let mut layer_hint_map = std::collections::HashMap::new();

    let num_keys = props.data.physical_layout.len();
    let num_layers = props.data.layers.len();

    for i in 0..num_keys {
        if i < hint_chars.len() * hint_chars.len() {
            let h = format!(
                "{}{}",
                hint_chars.chars().nth(i / hint_chars.len()).unwrap(),
                hint_chars.chars().nth(i % hint_chars.len()).unwrap()
            );
            hint_map.insert(h, i);
        }
    }
    for i in 0..num_layers {
        if i < hint_chars.len() {
            let h = format!("l{}", hint_chars.chars().nth(i).unwrap());
            layer_hint_map.insert(h, i);
        }
    }

    {
        let container_ref = container_ref.clone();
        use_effect(move || {
            if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
            || ()
        });
    }

    let on_keydown = {
        let jump_mode_active = jump_mode_active.clone();
        let jump_input = jump_input.clone();
        let selected_key = props.selected_key.clone();
        let current_layer = props.current_layer.clone();
        let hint_map = hint_map.clone();
        let layer_hint_map = layer_hint_map.clone();

        Callback::from(move |e: KeyboardEvent| {
            if selected_key.is_some() {
                return;
            }

            if *jump_mode_active {
                match e.key().as_str() {
                    "Escape" => {
                        jump_mode_active.set(false);
                        jump_input.set(String::new());
                        e.prevent_default();
                    }
                    key if key.len() == 1 && hint_chars.contains(key) => {
                        let mut new_input = (*jump_input).clone();
                        new_input.push_str(key);

                        if let Some(&idx) = hint_map.get(&new_input) {
                            selected_key.set(Some(SelectedKey {
                                layer_index: *current_layer,
                                key_index: idx,
                            }));
                            jump_mode_active.set(false);
                            jump_input.set(String::new());
                        } else if let Some(&l_idx) = layer_hint_map.get(&new_input) {
                            current_layer.set(l_idx);
                            jump_mode_active.set(false);
                            jump_input.set(String::new());
                        } else if hint_map.keys().any(|h| h.starts_with(&new_input)) || layer_hint_map.keys().any(|h| h.starts_with(&new_input)) {
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

    let layer = &props.data.layers[*props.current_layer];

    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    
    // Split into standard keys and aliases for rendering
    let num_standard_keys = props.data.defsrc.len() - props.data.aliases.len();
    
    for pk in props.data.physical_layout.iter() {
        min_x = min_x.min(pk.x);
        max_x = max_x.max(pk.x + pk.width);
        min_y = min_y.min(pk.y);
        max_y = max_y.max(pk.y + pk.height);
    }
    
    let scale = 0.05f32;
    let content_width = (max_x - min_x) as f32 * scale;
    let content_height = (max_y - min_y) as f32 * scale;
    let offset_x = -(min_x as f32 * scale);
    let offset_y = -(min_y as f32 * scale);

    let alias_y_threshold = 6500; // Based on our compute_standard_kanata_layout

    html! {
        <div ref={container_ref.clone()} tabindex="0" onkeydown={on_keydown} class="flex flex-col items-center w-full focus:outline-none">
            <div class="flex flex-wrap gap-2 mb-4 relative">
                { for props.data.layers.iter().enumerate().map(|(i, l)| {
                    let is_active = i == *props.current_layer;
                    let onclick = { let cl = props.current_layer.clone(); Callback::from(move |_| cl.set(i)) };
                    let hint = layer_hint_map.iter().find(|(_, &idx)| idx == i).map(|(h, _)| h);
                    let show_hint = *jump_mode_active && hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);
                    html! {
                        <div class="relative">
                            <button onclick={onclick} class={classes!("px-4", "py-1.5", "rounded-md", "shadow-sm", "font-medium", "transition-all", "relative",
                                if is_active { "bg-white dark:bg-gray-700 text-blue-600 dark:text-blue-400" } else { "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 bg-gray-100 dark:bg-gray-800" }
                            )}>
                                {&l.name}
                                { if show_hint {
                                    let h = hint.unwrap();
                                    let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                    html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                } else { html! {} }}
                            </button>
                        </div>
                    }
                })}
            </div>

            <div class={classes!("relative", "border", "dark:border-gray-600", "p-8", "rounded-xl", "bg-gray-50", "dark:bg-gray-800", "shadow-inner", "overflow-auto", "w-full", "max-w-full")} style="min-height: 350px; height: 65vh;">
                <div class="relative mx-auto" style={format!("width: {}px; height: {}px;", content_width, content_height)}>
                    { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| {
                        let binding = layer.bindings.get(i).cloned().unwrap_or_else(|| "".to_string());
                        let defsrc_name = props.data.defsrc.get(i).cloned().unwrap_or_default();
                        let parts = get_kanata_binding_parts(&binding, &props.data.aliases, is_mac(), props.is_laptop);
                        let x = (pk.x as f32 * scale + offset_x) as i32;
                        let y = (pk.y as f32 * scale + offset_y) as i32;
                        let w = (pk.width as f32 * scale) as i32 - 2;
                        let h = (pk.height as f32 * scale) as i32 - 2;
                        let onclick = { let sk = props.selected_key.clone(); let cur_l = *props.current_layer; Callback::from(move |_| sk.set(Some(SelectedKey { layer_index: cur_l, key_index: i }))) };
                        let hint = hint_map.iter().find(|(_, &idx)| idx == i).map(|(h, _)| h);
                        let show_hint = *jump_mode_active && hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);

                        let is_alias_section = pk.y >= alias_y_threshold;
                        let alias_name = if is_alias_section { Some(defsrc_name.clone()) } else { None };

                        html! {
                            <>
                                { if pk.y == alias_y_threshold && i == num_standard_keys {
                                    html! { <div class="absolute w-full border-t-2 border-dashed border-gray-300 dark:border-gray-600" style={format!("top: {}px; left: 0;", y - 20)}>
                                        <span class="absolute -top-3 left-0 bg-gray-50 dark:bg-gray-800 px-2 text-[10px] font-bold text-gray-400">{"ALIASES"}</span>
                                    </div> }
                                } else { html! {} }}
                                
                                { if let Some(name) = alias_name {
                                    html! { <div class="absolute text-[8px] font-bold text-blue-500 truncate text-center" style={format!("left: {}px; top: {}px; width: {}px;", x, y - 12, w)}> {name} </div> }
                                } else { html! {} }}

                                <div onclick={onclick} class={classes!("absolute", "bg-white", "dark:bg-gray-700", "border", "border-gray-300", "dark:border-gray-600", "flex", "flex-col", "items-center", "justify-center", "rounded", "cursor-pointer", "hover:border-blue-400", "dark:hover:border-blue-500", "shadow-sm", "transition-all", "select-none",
                                    if is_alias_section { "bg-blue-50/30 dark:bg-blue-900/10" } else { "" }
                                )} style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", x, y, w, h)}>

                                    <div class="w-full flex justify-between px-1 text-[7px] text-gray-400 absolute top-0.5 pointer-events-none">
                                        <span class="truncate max-w-[45%]">{parts.top_left}</span>
                                        <span class="truncate max-w-[45%] text-right">{parts.top_right}</span>
                                    </div>
                                    <span class="text-[12px] font-bold truncate px-1 mt-1 leading-tight text-center pointer-events-none">{parts.center}</span>
                                    
                                    <div class="w-full flex justify-end px-1 text-[6px] text-gray-300 dark:text-gray-500 absolute bottom-0.5 pointer-events-none font-mono">
                                        <span>{defsrc_name}</span>
                                    </div>

                                    { if show_hint {
                                        let h = hint.unwrap();
                                        let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                        html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                    } else { html! {} }}
                                </div>
                            </>
                        }
                    })}
                </div>
            </div>

            { if let Some(sk) = &*props.selected_key {
                let on_close = { 
                    let sk = props.selected_key.clone(); 
                    let container_ref = container_ref.clone();
                    Callback::from(move |_: MouseEvent| {
                        sk.set(None);
                        if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                            let _ = element.focus();
                        }
                    }) 
                };
                html! { <KanataBindingPopup data={props.data.clone()} selected_key={sk.clone()} on_close={on_close} on_update={props.on_update.clone()} is_laptop={props.is_laptop} /> }
            } else { html! {} }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PopupProps {
    data: KeymapData,
    selected_key: SelectedKey,
    on_close: Callback<MouseEvent>,
    on_update: Callback<KeymapData>,
    is_laptop: bool,
}

#[function_component]
fn KanataBindingPopup(props: &PopupProps) -> Html {
    let num_standard_keys = props.data.defsrc.len() - props.data.aliases.len();
    let is_alias_section = props.selected_key.key_index >= num_standard_keys;
    let binding = &props.data.layers[props.selected_key.layer_index].bindings[props.selected_key.key_index];
    
    let initial_text = if is_alias_section {
        // Alias section: just the value (RHS)
        props.data.aliases.get(binding).cloned().unwrap_or_else(|| binding.clone())
    } else if binding.starts_with('@') {
        // Binding is an alias: alias = value
        let name = &binding[1..];
        if let Some(val) = props.data.aliases.get(name) {
            format!("{} = {}", name, val)
        } else {
            binding.clone()
        }
    } else {
        binding.clone()
    };

    let current_text = use_state(|| initial_text);
    let suggestion_index = use_state(|| 0usize);
    let show_suggestions = use_state(|| true);
    let input_ref = use_node_ref();

    let is_valid = {
        let validator = KanataValidator::new(&props.data);
        validator.validate_full(&*current_text)
    };

    let on_save = {
        let on_update = props.on_update.clone();
        let data = props.data.clone();
        let sk = props.selected_key.clone();
        let current_text = current_text.clone();
        let on_close = props.on_close.clone();
        let is_valid = is_valid;
        let is_laptop = props.is_laptop;
        Callback::from(move |e| {
            if !is_valid { return; }
            let mut new_data = data.clone();
            let text = (*current_text).clone().trim().to_string();
            
            let num_standard_keys = new_data.defsrc.len() - new_data.aliases.len();
            let is_alias_section = sk.key_index >= num_standard_keys;

            if is_alias_section {
                // Editing an existing alias value (RHS)
                let name = new_data.defsrc[sk.key_index].clone();
                new_data.aliases.insert(name, text);
            } else if let Some((name, val)) = text.split_once('=') {
                // Creating or updating an alias (name = val)
                let name = name.trim();
                let val = val.trim();
                if !name.is_empty() && !val.is_empty() {
                    new_data.aliases.insert(name.to_string(), val.to_string());
                    new_data.layers[sk.layer_index].bindings[sk.key_index] = format!("@{}", name);
                }
            } else {
                // Normal binding
                new_data.layers[sk.layer_index].bindings[sk.key_index] = text;
            }

            // Recompute everything to ensure consistency
            let key_names: Vec<String> = new_data.defsrc.iter().take(num_standard_keys).cloned().collect();
            let mut sorted_alias_names: Vec<String> = new_data.aliases.keys().cloned().collect();
            sorted_alias_names.sort();
            
            new_data.defsrc = key_names.clone();
            new_data.defsrc.extend(sorted_alias_names.clone());
            new_data.physical_layout = layout::compute_standard_kanata_layout(&key_names, &sorted_alias_names, is_mac(), is_laptop);
            
            for layer in &mut new_data.layers {
                let current_bindings = layer.bindings.clone();
                layer.bindings = current_bindings.into_iter().take(num_standard_keys).collect();
                layer.bindings.extend(sorted_alias_names.clone());
            }

            on_update.emit(new_data);
            on_close.emit(e);
        })
    };

    let text = (*current_text).clone();
    let mut suggestions = Vec::new();
    let lower_text = text.to_lowercase();
    
    // Determine the relevant part of the string for completion
    let (prefix, query) = if let Some(last_open) = lower_text.rfind('(') {
        // We are inside an action
        let after_open = &lower_text[last_open + 1..];
        
        let mut depth = 0;
        let mut last_space = None;
        for (i, c) in after_open.char_indices() {
            if c == '(' { depth += 1; }
            else if c == ')' { depth -= 1; }
            else if c.is_whitespace() && depth == 0 { last_space = Some(i); }
        }

        if let Some(space_idx) = last_space {
            // We are in parameters
            let query = &after_open[space_idx + 1..];
            (&text[..last_open + 1 + space_idx + 1], query)
        } else {
            // Completing the action name itself
            (&text[..last_open + 1], after_open)
        }
    } else if let Some(eq_pos) = lower_text.find('=') {
        let after_eq = &lower_text[eq_pos + 1..];
        let trimmed = after_eq.trim_start();
        let offset = lower_text.len() - trimmed.len();
        (&text[..offset], trimmed)
    } else {
        ("", lower_text.as_str())
    };

    let is_layer_action = if let Some(last_open) = lower_text.rfind('(') {
        let after_open = &lower_text[last_open + 1..];
        let parts: Vec<&str> = after_open.split_whitespace().collect();
        let has_space = after_open.chars().any(|c| c.is_whitespace());
        !parts.is_empty() && (parts[0] == "layer-toggle" || parts[0] == "layer-switch" || parts[0] == "layer-while-held") && has_space
    } else {
        false
    };

    if is_layer_action {
        for layer in &props.data.layers {
            if layer.name.to_lowercase().contains(query) {
                suggestions.push(layer.name.clone());
            }
        }
    } else {
        let only_actions = query.starts_with('(') || (text.contains('=') && query.is_empty());
        let clean_query = if query.starts_with('(') { &query[1..] } else { query };

        for action in KANATA_ACTIONS {
            if action.name.contains(clean_query) {
                suggestions.push(format!("({}", action.name));
            }
        }
        if !only_actions {
            for key in KANATA_KEYS {
                if key.contains(query) {
                    suggestions.push(key.to_string());
                }
            }
            for alias in props.data.aliases.keys() {
                if alias.contains(query) {
                    suggestions.push(alias.clone());
                }
            }
        }
    }

    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(30);

    let on_keydown = {
        let on_close = props.on_close.clone();
        let on_save = on_save.clone();
        let suggestion_index = suggestion_index.clone();
        let suggestions = suggestions.clone();
        let current_text = current_text.clone();
        let show_suggestions = show_suggestions.clone();
        let prefix = prefix.to_string();
        Callback::from(move |e: KeyboardEvent| {
            match e.key().as_str() {
                "Escape" => {
                    e.prevent_default();
                    on_close.emit(MouseEvent::new("click").unwrap());
                }
                "Enter" => {
                    e.prevent_default();
                    on_save.emit(MouseEvent::new("click").unwrap().into());
                }
                "ArrowDown" => {
                    if !suggestions.is_empty() {
                        e.prevent_default();
                        suggestion_index.set((*suggestion_index + 1) % suggestions.len());
                        show_suggestions.set(true);
                    }
                }
                "ArrowUp" => {
                    if !suggestions.is_empty() {
                        e.prevent_default();
                        let new_idx = if *suggestion_index == 0 { suggestions.len() - 1 } else { *suggestion_index - 1 };
                        suggestion_index.set(new_idx);
                        show_suggestions.set(true);
                    }
                }
                "Tab" if *show_suggestions && !suggestions.is_empty() => {
                    e.prevent_default();
                    let mut completed = suggestions[*suggestion_index].clone();
                    if prefix.ends_with('(') && completed.starts_with('(') {
                        completed = completed[1..].to_string();
                    }
                    current_text.set(format!("{}{}", prefix, completed));
                    show_suggestions.set(false);
                }
                _ => {}
            }
        })
    };

    {
        let input_ref = input_ref.clone();
        use_effect_with((), move |_| {
            let timeout_cb = Closure::wrap(Box::new(move || {
                if let Some(input) = input_ref.cast::<web_sys::HtmlInputElement>() {
                    let _ = input.focus();
                    let _ = input.select();
                }
            }) as Box<dyn FnMut()>);
            let window = web_sys::window().expect("should have a window");
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_cb.as_ref().unchecked_ref(),
                50,
            );
            move || {
                drop(timeout_cb);
            }
        });
    }

    let mut info_title = "Binding".to_string();
    let mut info_desc = "Enter a keycode or an alias name.".to_string();
    let mut info_params = Vec::new();

    if let Some((name, val)) = text.split_once('=') {
        info_title = format!("New Alias: {}", name.trim());
        let val = val.trim();
        if val.starts_with('(') {
            let inner = if val.ends_with(')') { &val[1..val.len()-1] } else { &val[1..] };
            let parts: Vec<&str> = inner.split_whitespace().collect();
            if !parts.is_empty() {
                if let Some(action) = KANATA_ACTIONS.iter().find(|a| a.name == parts[0]) {
                    info_desc = action.description.to_string();
                    info_params = action.params.iter().map(|p| match p {
                        ParamType::Timeout => "timeout",
                        ParamType::Action => "action",
                        ParamType::Layer => "layer",
                        ParamType::Any => "any",
                    }).collect();
                }
            }
        }
    } else if text.starts_with('(') {
        let inner = if text.ends_with(')') { &text[1..text.len()-1] } else { &text[1..] };
        let parts: Vec<&str> = inner.split_whitespace().collect();
        if !parts.is_empty() {
            if let Some(action) = KANATA_ACTIONS.iter().find(|a| a.name == parts[0]) {
                info_title = action.name.to_uppercase();
                info_desc = action.description.to_string();
                info_params = action.params.iter().map(|p| match p {
                    ParamType::Timeout => "timeout",
                    ParamType::Action => "action",
                    ParamType::Layer => "layer",
                    ParamType::Any => "any",
                }).collect();
            }
        }
    } else if let Some(aliased) = props.data.aliases.get(&text) {
        info_title = format!("Alias: {}", text);
        info_desc = aliased.clone();
    }

    html! {
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50" onkeydown={on_keydown}>
            <div class="bg-white dark:bg-gray-800 p-6 rounded-lg shadow-xl w-full max-w-6xl flex flex-col h-[80vh]">
                <h3 class="text-xl font-bold mb-4">{"Edit Kanata Binding"}</h3>
                
                <div class="flex flex-1 overflow-hidden">
                    <div class="w-1/4 pr-4 border-r dark:border-gray-700 h-full">
                        <div class="bg-blue-50 dark:bg-blue-900/20 p-4 rounded-lg h-full overflow-y-auto">
                            <h4 class="font-bold text-blue-600 dark:text-blue-400 mb-2">{"Instructions"}</h4>
                            <div class="text-xs space-y-3">
                                <div>
                                    <p class="font-semibold">{"Define Alias:"}</p>
                                    <code class="block bg-gray-100 dark:bg-gray-800 p-1 rounded mt-1">{"name = (action ...)"}</code>
                                    <p class="mt-1 opacity-70">{"e.g. entctl = (tap-hold 200 200 ent lctl)"}</p>
                                </div>
                                <div>
                                    <p class="font-semibold">{"Tap-Hold:"}</p>
                                    <p class="opacity-70">{"(tap-hold tap-timeout hold-timeout tap-action hold-action)"}</p>
                                </div>
                                <div>
                                    <p class="font-semibold">{"Layers:"}</p>
                                    <p class="opacity-70">{"(layer-toggle layer-name)"}</p>
                                    <p class="opacity-70">{"(layer-switch layer-name)"}</p>
                                </div>
                                <div>
                                    <p class="font-semibold">{"Multi-Action:"}</p>
                                    <p class="opacity-70">{"(multi action1 action2 ...)"}</p>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="flex-1 px-4 flex flex-col items-center h-full">
                        <div class="w-full max-w-lg flex flex-col h-full overflow-hidden">
                            <input ref={input_ref} value={(*current_text).clone()} oninput={
                                let current_text = current_text.clone();
                                let show_suggestions = show_suggestions.clone();
                                let suggestion_index = suggestion_index.clone();
                                Callback::from(move |e: InputEvent| {
                                    let input: web_sys::HtmlInputElement = e.target().unwrap().unchecked_into();
                                    current_text.set(input.value());
                                    show_suggestions.set(true);
                                    suggestion_index.set(0);
                                })
                            } class="w-full p-4 border-2 border-blue-200 dark:border-blue-800 rounded-xl mb-6 dark:bg-gray-900 text-xl font-mono focus:border-blue-500 focus:outline-none shadow-sm" 
                            placeholder="esc, cap1, or name = (action ...)" />
                            
                            <div class="bg-gray-50 dark:bg-gray-900 p-6 rounded-xl border dark:border-gray-700 flex-1 shadow-inner overflow-y-auto">
                                <h4 class="font-bold text-blue-500 mb-3 text-lg">{info_title}</h4>
                                <p class="text-sm text-gray-600 dark:text-gray-400 mb-6 leading-relaxed">{info_desc}</p>
                                { if !info_params.is_empty() {
                                    html! {
                                        <div class="text-sm">
                                            <div class="font-bold mb-2 text-gray-500 uppercase tracking-wider text-xs">{"Parameter Structure"}</div>
                                            <div class="flex flex-wrap gap-2">
                                                { for info_params.iter().map(|p| html! { 
                                                    <span class="bg-white dark:bg-gray-800 border dark:border-gray-700 px-2 py-1 rounded text-xs font-mono">
                                                        {p}
                                                    </span> 
                                                })}
                                            </div>
                                        </div>
                                    }
                                } else { html! {} }}
                            </div>
                        </div>
                    </div>

                    <div class="w-1/4 pl-4 border-l dark:border-gray-700 h-full">
                        <div class="flex flex-col h-full overflow-hidden">
                            <h4 class="font-bold mb-3 text-sm text-gray-500 uppercase tracking-wider">{"Suggestions"}</h4>
                            <div class="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                                <div class="grid grid-cols-1 gap-1.5">
                                    { for suggestions.iter().enumerate().map(|(i, s)| {
                                        let s_clone = s.clone();
                                        let current_text = current_text.clone();
                                        let show_suggestions = show_suggestions.clone();
                                        let is_active = i == *suggestion_index && *show_suggestions;
                                        let onclick = Callback::from(move |_| {
                                            current_text.set(s_clone.clone());
                                            show_suggestions.set(false);
                                        });
                                        html! {
                                            <button onclick={onclick} class={classes!("text-left", "px-3", "py-2", "text-xs", "rounded-lg", "transition-colors", "font-mono", "border", "truncate",
                                                if is_active { "bg-blue-600 text-white border-blue-400" } else { "hover:bg-blue-50 dark:hover:bg-blue-900/30 hover:text-blue-600 dark:hover:text-blue-400 border-transparent hover:border-blue-200 dark:hover:border-blue-800 text-gray-700 dark:text-gray-300" }
                                            )}>
                                                {s}
                                            </button>
                                        }
                                    })}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="flex justify-end space-x-3 mt-8 pt-4 border-t dark:border-gray-700">
                    <button onclick={props.on_close.clone()} class="px-8 py-2.5 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200 font-medium rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors">{"Cancel"}</button>
                    <button onclick={on_save} disabled={!is_valid} class={classes!("px-8", "py-2.5", "text-white", "font-medium", "rounded-lg", "transition-all",
                        if is_valid { "bg-blue-600 hover:bg-blue-700 shadow-md shadow-blue-500/20" } else { "bg-gray-400 cursor-not-allowed" }
                    )}>{"Apply Binding"}</button>
                </div>
            </div>
        </div>
    }
}
