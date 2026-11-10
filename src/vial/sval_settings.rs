//! Keyboard settings panel for SVIL boards, rendered from the VIA3 custom UI
//! `menus` of the keyboard definition (pointing devices, layer colors, ...).

use std::collections::HashMap;

use serde_json::Value;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use vial_protocol::svil::keycode_names::KeycodeNames;
use vial_protocol::svil::menus::{self, CustomValueEntry, ValueRef};

#[derive(Properties, PartialEq)]
pub struct SvalSettingsProps {
    pub menus: Value,
    pub values: Vec<CustomValueEntry>,
    pub names: KeycodeNames,
    /// A control changed: where its value lives and its new raw bytes.
    pub on_change: Callback<(ValueRef, Vec<u8>)>,
}

#[function_component]
pub fn SvalSettingsPanel(props: &SvalSettingsProps) -> Html {
    let values: HashMap<String, u32> =
        props.values.iter().map(|e| (e.key.clone(), menus::bytes_to_int(&e.data))).collect();
    let top: Vec<&Value> = props.menus.as_array().map(|a| a.iter().filter(|v| v.is_object()).collect()).unwrap_or_default();

    if top.is_empty() {
        return html! { <p class="text-gray-500">{"This keyboard has no settings menus."}</p> };
    }

    html! {
        <div class="w-full max-w-3xl mx-auto space-y-6">
            <h2 class="text-xl font-bold">{"Keyboard Settings"}</h2>
            <p class="text-sm text-gray-500">{"Changes are written to the keyboard and saved immediately."}</p>
            { for top.into_iter().filter(|i| menus::is_shown(i, &values)).map(|item| html! {
                <div class="p-5 bg-gray-50 dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700">
                    <h3 class="text-lg font-semibold mb-3">{menus::label(item)}</h3>
                    { render_items(&menus::children(item), &values, props, 0) }
                </div>
            })}
        </div>
    }
}

fn render_items(items: &[&Value], values: &HashMap<String, u32>, props: &SvalSettingsProps, depth: usize) -> Html {
    html! {
        <div class="space-y-3">
            { for items.iter().filter(|i| menus::is_shown(i, values)).map(|item| {
                match menus::value_ref(item) {
                    Some(r) => render_control(item, r, values, props),
                    None => html! {
                        <div class={classes!(if depth == 0 { "pt-2" } else { "pl-4 border-l border-gray-200 dark:border-gray-700" })}>
                            <h4 class="text-sm font-bold text-gray-500 uppercase tracking-wide mb-2">{menus::label(item)}</h4>
                            { render_items(&menus::children(item), values, props, depth + 1) }
                        </div>
                    },
                }
            })}
        </div>
    }
}

fn render_control(item: &Value, r: ValueRef, values: &HashMap<String, u32>, props: &SvalSettingsProps) -> Html {
    let width = menus::byte_width(item);
    let current = values.get(&r.key).copied();
    let label = menus::label(item).to_string();
    let emit = {
        let cb = props.on_change.clone();
        let r = r.clone();
        move |v: u32| cb.emit((r.clone(), menus::int_to_bytes(v, width)))
    };
    let Some(current) = current else {
        return html! {
            <div class="flex justify-between text-sm">
                <span>{label}</span><span class="text-gray-400 italic">{"not read"}</span>
            </div>
        };
    };
    let row = |control: Html| -> Html {
        html! {
            <div class="flex items-center justify-between gap-4 text-sm">
                <label class="flex-1">{label.clone()}</label>
                <div class="flex items-center gap-2">{control}</div>
            </div>
        }
    };

    match menus::item_type(item) {
        "range" => {
            let (min, max) = menus::range(item);
            let onchange = Callback::from(move |e: Event| {
                if let Some(input) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                    if let Ok(v) = input.value().parse::<i64>() {
                        emit(v.clamp(min, max) as u32);
                    }
                }
            });
            row(html! {
                <>
                    <input type="range" min={min.to_string()} max={max.to_string()} value={current.to_string()}
                        class="w-48" onchange={onchange.clone()} />
                    <input type="number" min={min.to_string()} max={max.to_string()} value={current.to_string()}
                        class="w-20 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900" onchange={onchange} />
                </>
            })
        }
        "toggle" => {
            let (off, on) = menus::toggle_values(item);
            let onchange = Callback::from(move |e: Event| {
                if let Some(input) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                    emit(if input.checked() { on } else { off });
                }
            });
            row(html! { <input type="checkbox" class="w-5 h-5" checked={current == on} onchange={onchange} /> })
        }
        "dropdown" => {
            let options = menus::dropdown_options(item);
            let onchange = Callback::from(move |e: Event| {
                // <select>.value, read via Reflect (no HtmlSelectElement binding needed).
                let value = e
                    .target()
                    .and_then(|t| js_sys::Reflect::get(&t, &"value".into()).ok())
                    .and_then(|v| v.as_string());
                if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                    emit(v);
                }
            });
            row(html! {
                <select class="px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900" onchange={onchange}>
                    { for options.into_iter().map(|(l, v)| html! {
                        <option value={v.to_string()} selected={v == current}>{l}</option>
                    })}
                </select>
            })
        }
        "color" => {
            let hue = current & 0xFF;
            let sat = (current >> 8) & 0xFF;
            let swatch = format!(
                "background: hsl({}, {}%, 50%); width: 1.5rem; height: 1.5rem; border-radius: 0.375rem;",
                hue * 360 / 255,
                sat * 100 / 255
            );
            let on_hue = {
                let emit = emit.clone();
                Callback::from(move |e: Event| {
                    if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                        let h = i.value().parse::<u32>().unwrap_or(0).min(255);
                        emit(h | (sat << 8));
                    }
                })
            };
            let on_sat = Callback::from(move |e: Event| {
                if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                    let s = i.value().parse::<u32>().unwrap_or(0).min(255);
                    emit(hue | (s << 8));
                }
            });
            row(html! {
                <>
                    <span class="text-xs text-gray-500">{"H"}</span>
                    <input type="range" min="0" max="255" value={hue.to_string()} class="w-28" onchange={on_hue} />
                    <span class="text-xs text-gray-500">{"S"}</span>
                    <input type="range" min="0" max="255" value={sat.to_string()} class="w-28" onchange={on_sat} />
                    <div style={swatch}></div>
                </>
            })
        }
        "keycode" => {
            let names = props.names.clone();
            let text = names.stringify(current as u16);
            let onchange = Callback::from(move |e: Event| {
                if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                    match names.parse(&i.value()) {
                        Some(kc) => emit(kc as u32),
                        None => i.set_value(&names.stringify(current as u16)),
                    }
                }
            });
            row(html! {
                <input type="text" value={text} class="w-48 px-2 py-1 font-mono rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900"
                    onchange={onchange} />
            })
        }
        "button" => {
            let onclick = Callback::from(move |_: MouseEvent| emit(1));
            row(html! {
                <button class="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white rounded" onclick={onclick}>{"Run"}</button>
            })
        }
        other => row(html! { <span class="text-gray-400 font-mono">{format!("{other}: {current}")}</span> }),
    }
}
