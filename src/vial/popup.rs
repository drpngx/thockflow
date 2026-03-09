use web_sys::HtmlInputElement;
use yew::prelude::*;

use vial_protocol::keycodes::{keycode_from_string, keycode_label, VIAL_SUGGESTIONS};

#[derive(Properties, PartialEq)]
pub struct VialKeyBindingPopupProps {
    pub current_keycode: u16,
    pub protocol_version: u32,
    pub keyboard_name: String,
    pub on_close: Callback<MouseEvent>,
    pub on_apply: Callback<u16>,
}

const MOD_LIST: &[&str] = &["LCtrl", "LShift", "LAlt", "LGui", "RCtrl", "RShift", "RAlt", "RGui"];

#[function_component(VialKeyBindingPopup)]
pub fn vial_key_binding_popup(props: &VialKeyBindingPopupProps) -> Html {
    let current_text = use_state(|| keycode_label(props.current_keycode, props.protocol_version, &props.keyboard_name));
    let show_suggestions = use_state(|| false);
    let suggestion_index = use_state(|| 0usize);
    let _is_valid = use_state(|| true);

    let input_ref = use_node_ref();

    {
        let input_ref = input_ref.clone();
        use_effect_with((), move |_| {
            if let Some(input) = input_ref.cast::<web_sys::HtmlInputElement>() {
                let _ = input.focus();
                let _ = input.select();
            }
            || ()
        });
    }

    let val_upper = (*current_text).to_uppercase();
    let mut suggestions: Vec<String> = Vec::new();

    if val_upper.is_empty() {
        suggestions.push("MO(".to_string());
        suggestions.push("LT(".to_string());
        suggestions.push("MT(".to_string());
        suggestions.push("TO(".to_string());
        suggestions.push("TG(".to_string());
    }

    // Custom dynamic suggestions for prefixes
    let prefixes = ["MO", "LT", "TO", "TG", "TT", "OSL", "DF", "MT"];
    for &p in &prefixes {
        if val_upper == p {
            suggestions.push(format!("{}(", p));
        }
    }

    if val_upper.ends_with('(') {
        let prefix = &val_upper[..val_upper.len() - 1];
        if ["MO", "TO", "TG", "TT", "OSL", "DF"].contains(&prefix) {
            for i in 0..32 {
                suggestions.push(format!("{}({})", prefix, i));
            }
        } else if prefix == "LT" {
             for i in 0..16 {
                suggestions.push(format!("{}({},", prefix, i));
            }
        } else if prefix == "MT" {
            for &m in MOD_LIST {
                suggestions.push(format!("{}({},", prefix, m));
            }
        }
    } else if val_upper.contains('(') && val_upper.contains(',') && !val_upper.ends_with(')') {
        // LT(5, ... or MT(LCtrl, ...
        let parts: Vec<&str> = val_upper.splitn(2, '(').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            if let Some((first_arg, _)) = parts[1].split_once(',') {
                let first_arg = first_arg.trim();
                // suggest all basic keys for the tap part
                for &s in VIAL_SUGGESTIONS {
                    if !s.contains('(') && !s.contains(' ') { // basic keys only
                        suggestions.push(format!("{}({},{})", prefix, first_arg, s));
                        suggestions.push(format!("{}({}, {})", prefix, first_arg, s));
                    }
                }
            }
        }
    }

    let mut suggestions: Vec<String> = suggestions
        .into_iter()
        .filter(|s| s.to_uppercase().starts_with(&val_upper))
        .collect();

    if suggestions.len() < 20 {
        let more: Vec<String> = VIAL_SUGGESTIONS
            .iter()
            .filter(|&&s| s.to_uppercase().starts_with(&val_upper) || s.to_uppercase().contains(&val_upper))
            .map(|&s| s.to_string())
            .collect();
        
        for s in more {
            if !suggestions.contains(&s) {
                suggestions.push(s);
                if suggestions.len() >= 20 { break; }
            }
        }
    }

    if suggestions.is_empty() {
        if let Some(_kc) = keycode_from_string(&*current_text, props.protocol_version, &props.keyboard_name) {
            suggestions.push("".to_string()); // Valid custom parsing
        }
    }

    let on_apply = {
        let text = (*current_text).clone();
        let on_apply_prop = props.on_apply.clone();
        let protocol_version = props.protocol_version;
        let keyboard_name = props.keyboard_name.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(kc) = keycode_from_string(&text, protocol_version, &keyboard_name) {
                on_apply_prop.emit(kc);
            }
        })
    };

    let on_keydown = {
        let text = (*current_text).clone();
        let suggestions = suggestions.clone();
        let show_suggestions = show_suggestions.clone();
        let suggestion_index = suggestion_index.clone();
        let on_apply_prop = props.on_apply.clone();
        let current_text = current_text.clone();
        let on_close = props.on_close.clone();
        let protocol_version = props.protocol_version;
        let keyboard_name = props.keyboard_name.clone();

        Callback::from(move |e: KeyboardEvent| match e.key().as_str() {
            " " | "(" => {
                let text_up = text.to_uppercase();
                if ["MO", "LT", "TO", "TG", "TT", "OSL", "DF", "MT"].contains(&text_up.as_str()) {
                    current_text.set(format!("{}(", text_up));
                    e.prevent_default();
                    return;
                }
            }
            "ArrowDown" => {
                if *show_suggestions && !suggestions.is_empty() {
                    let mut next = *suggestion_index + 1;
                    if next >= suggestions.len() {
                        next = suggestions.len() - 1;
                    }
                    suggestion_index.set(next);
                    e.prevent_default();
                }
            }
            "ArrowUp" => {
                if *show_suggestions && !suggestions.is_empty() {
                    let next = suggestion_index.saturating_sub(1);
                    suggestion_index.set(next);
                    e.prevent_default();
                }
            }
            "Enter" | "Tab" => {
                if *show_suggestions && !suggestions.is_empty() {
                    let sel = &suggestions[*suggestion_index];
                    if !sel.is_empty() {
                        current_text.set(sel.to_string());
                        show_suggestions.set(false);
                        e.prevent_default();
                        return;
                    }
                }
                if e.key() == "Enter" {
                    if let Some(kc) = keycode_from_string(&text, protocol_version, &keyboard_name) {
                        on_apply_prop.emit(kc);
                    }
                }
                e.prevent_default();
            }
            "Escape" => {
                if *show_suggestions {
                    show_suggestions.set(false);
                } else {
                    on_close.emit(MouseEvent::new("click").unwrap());
                }
                e.prevent_default();
            }
            _ => {
                show_suggestions.set(true);
                suggestion_index.set(0);
            }
        })
    };

    let current_kc_parsed = keycode_from_string(&*current_text, props.protocol_version, &props.keyboard_name);

    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4" onclick={props.on_close.clone()}>
            <div class="bg-[#1a202c] text-white rounded-lg shadow-2xl flex max-w-4xl w-full overflow-hidden border border-gray-700 h-[60vh]" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                
                <div class="flex-1 p-8 overflow-y-auto flex flex-col">
                    <h2 class="text-2xl font-bold mb-4">{"Edit Key"}</h2>
                    
                    <input
                        ref={input_ref}
                        type="text"
                        class="w-full bg-gray-900 border border-gray-600 focus:border-blue-500 text-2xl p-4 rounded font-mono focus:outline-none mb-4"
                        value={(*current_text).clone()}
                        oninput={let current_text = current_text.clone(); Callback::from(move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            current_text.set(input.value());
                        })}
                        onkeydown={on_keydown}
                    />

                    <div class="mt-4 p-4 bg-gray-800 rounded-lg border border-gray-700">
                        <div class="text-sm text-gray-400 mb-2 font-bold uppercase tracking-widest">{"Keypress Info"}</div>
                        { if let Some(kc) = current_kc_parsed {
                            html! {
                                <div>
                                    <span class="text-green-400 font-mono text-lg">{"Valid"}</span>
                                    <span class="text-gray-300 ml-4 font-mono">{format!("0x{:04X}", kc)}</span>
                                </div>
                            }
                        } else {
                            html! {
                                <div>
                                    <span class="text-red-400 font-mono text-lg">{"Invalid Keycode"}</span>
                                </div>
                            }
                        } }
                    </div>

                    <div class="flex justify-end space-x-4 mt-auto pt-8">
                        <button onclick={props.on_close.clone()} class="bg-gray-700 hover:bg-gray-600 text-white px-6 py-2 rounded font-semibold transition-colors">
                            {"Cancel"}
                        </button>
                        <button onclick={on_apply} class={classes!("px-6", "py-2", "rounded", "font-semibold", "transition-colors", if current_kc_parsed.is_some() { vec!["bg-green-600", "hover:bg-green-700", "text-white"] } else { vec!["bg-gray-800", "text-gray-500", "cursor-not-allowed"] })} disabled={current_kc_parsed.is_none()}>
                            {"Apply"}
                        </button>
                    </div>
                </div>

                <div class="w-80 bg-black border-l border-gray-700 flex flex-col h-full">
                    <div class="p-4 border-b border-gray-800 text-gray-400 text-xs font-bold uppercase tracking-widest shrink-0">{"Suggestions"}</div>
                    <div class="flex-1 overflow-y-auto">
                        { if *show_suggestions && !suggestions.is_empty() {
                            html! {
                                <div>
                                    { for suggestions.iter().enumerate().map(|(i, s)| {
                                        let is_active = i == *suggestion_index;
                                        let onclick = {
                                            let current_text = current_text.clone();
                                            let show_suggestions = show_suggestions.clone();
                                            let s = s.clone();
                                            Callback::from(move |_| {
                                                current_text.set(s.clone());
                                                show_suggestions.set(false);
                                            })
                                        };
                                        html! {
                                            <div onclick={onclick} class={classes!("p-3", "border-b", "border-gray-900", "cursor-pointer", "hover:bg-gray-800", "font-mono", if is_active { vec!["bg-blue-900", "text-white"] } else { vec!["text-gray-300"] })}>
                                                {s}
                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        } else { html! {} } }
                    </div>
                </div>

            </div>
        </div>
    }
}
