//! QMK Settings sub-component for the Vial tab.
//!
//! Renders settings grouped by tab, with proper boolean-bit and integer controls,
//! driven by the static definition table in `vial_protocol::qmk_settings`.

use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use super::QmkSettingValue;
use vial_protocol::qmk_settings::{self as qs, FieldType, QmkSettingDef};

#[derive(Properties, PartialEq)]
pub struct QmkSettingsProps {
    pub settings: Vec<QmkSettingValue>,
    pub on_change: Callback<(u16, Vec<u8>)>,
    pub on_reset: Callback<()>,
    pub loading: bool,
}

#[function_component]
pub fn QmkSettingsPanel(props: &QmkSettingsProps) -> Html {
    // Which qsids does the keyboard actually support?
    let supported_qsids: Vec<u16> = props.settings.iter().map(|s| s.qsid).collect();

    // Collect tabs that have at least one supported field.
    let tabs: Vec<&str> = qs::tab_names()
        .into_iter()
        .filter(|tab| {
            qs::QMK_SETTINGS
                .iter()
                .any(|d| d.tab == *tab && supported_qsids.contains(&d.qsid))
        })
        .collect();

    if props.settings.is_empty() {
        return html! {
            <div class="w-full max-w-2xl mx-auto">
                <p class="text-gray-500">{"No QMK settings available on this keyboard."}</p>
            </div>
        };
    }

    html! {
        <div class="w-full max-w-2xl mx-auto">
            <div class="flex justify-between items-center mb-4">
                <h2 class="text-xl font-bold">{"QMK Settings"}</h2>
                <button
                    class="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
                    onclick={props.on_reset.reform(|_| ())}
                    disabled={props.loading}
                >
                    {"Reset All"}
                </button>
            </div>

            { for tabs.iter().map(|&tab| {
                render_tab(tab, &props.settings, &supported_qsids, &props.on_change)
            })}
        </div>
    }
}

fn render_tab(
    tab: &str,
    settings: &[QmkSettingValue],
    supported_qsids: &[u16],
    on_change: &Callback<(u16, Vec<u8>)>,
) -> Html {
    let fields: Vec<&QmkSettingDef> = qs::QMK_SETTINGS
        .iter()
        .filter(|d| d.tab == tab && supported_qsids.contains(&d.qsid))
        .collect();

    if fields.is_empty() {
        return html! {};
    }

    html! {
        <div class="mb-6">
            <h3 class="text-lg font-semibold mb-2 text-gray-700 dark:text-gray-300">{tab}</h3>
            <div class="space-y-2">
                { for fields.iter().map(|def| {
                    render_field(def, settings, on_change)
                })}
            </div>
        </div>
    }
}

fn render_field(
    def: &QmkSettingDef,
    settings: &[QmkSettingValue],
    on_change: &Callback<(u16, Vec<u8>)>,
) -> Html {
    let value = settings.iter().find(|s| s.qsid == def.qsid);
    let raw = value.map(|v| v.value.clone()).unwrap_or_default();

    match &def.field_type {
        FieldType::Boolean { bit } => {
            let checked = qs::read_bool(&raw, *bit);
            let qsid = def.qsid;
            let bit = *bit;
            let raw_clone = raw.clone();
            let on_change = on_change.clone();
            html! {
                <div class="flex items-center justify-between p-3 bg-gray-100 dark:bg-gray-800 rounded">
                    <label class="text-sm">{def.title}</label>
                    <input
                        type="checkbox"
                        checked={checked}
                        class="w-5 h-5"
                        onchange={Callback::from(move |e: Event| {
                            let input: HtmlInputElement = e.target().unwrap().unchecked_into();
                            let mut new_val = raw_clone.clone();
                            qs::write_bool(&mut new_val, bit, input.checked());
                            on_change.emit((qsid, new_val));
                        })}
                    />
                </div>
            }
        }
        FieldType::Integer { min, max } => {
            let current = qs::read_integer(&raw, def.width);
            let qsid = def.qsid;
            let width = def.width;
            let on_change = on_change.clone();
            let min_val = *min;
            let max_val = *max;
            html! {
                <div class="flex items-center justify-between p-3 bg-gray-100 dark:bg-gray-800 rounded">
                    <label class="text-sm">{def.title}</label>
                    <input
                        type="number"
                        value={current.to_string()}
                        min={min_val.to_string()}
                        max={max_val.to_string()}
                        class="w-28 px-2 py-1 border rounded dark:bg-gray-700 dark:border-gray-600 text-sm"
                        onchange={Callback::from(move |e: Event| {
                            let input: HtmlInputElement = e.target().unwrap().unchecked_into();
                            let num: u32 = input.value().parse().unwrap_or(0)
                                .max(min_val).min(max_val);
                            let mut new_val = vec![0u8; width as usize];
                            qs::write_integer(&mut new_val, width, num);
                            on_change.emit((qsid, new_val));
                        })}
                    />
                </div>
            }
        }
        FieldType::ColorHsv => {
            let h = raw.first().copied().unwrap_or(0);
            let s = raw.get(1).copied().unwrap_or(0);
            let v = raw.get(2).copied().unwrap_or(0);
            let qsid = def.qsid;
            let on_change_h = on_change.clone();
            let on_change_s = on_change.clone();
            let on_change_v = on_change.clone();
            let raw_h = raw.clone();
            let raw_s = raw.clone();
            let raw_v = raw.clone();
            html! {
                <div class="flex items-center justify-between p-3 bg-gray-100 dark:bg-gray-800 rounded gap-2">
                    <label class="text-sm">{def.title}</label>
                    <div class="flex items-center gap-2 text-xs">
                        <label>{"H"}</label>
                        <input type="number" value={h.to_string()} min="0" max="255"
                            class="w-16 px-1 py-1 border rounded dark:bg-gray-700 dark:border-gray-600 text-sm"
                            onchange={Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target().unwrap().unchecked_into();
                                let val: u8 = input.value().parse().unwrap_or(0);
                                let mut nv = raw_h.clone();
                                if nv.is_empty() { nv.resize(3, 0); }
                                nv[0] = val;
                                on_change_h.emit((qsid, nv));
                            })}
                        />
                        <label>{"S"}</label>
                        <input type="number" value={s.to_string()} min="0" max="255"
                            class="w-16 px-1 py-1 border rounded dark:bg-gray-700 dark:border-gray-600 text-sm"
                            onchange={Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target().unwrap().unchecked_into();
                                let val: u8 = input.value().parse().unwrap_or(0);
                                let mut nv = raw_s.clone();
                                if nv.is_empty() { nv.resize(3, 0); }
                                nv[1] = val;
                                on_change_s.emit((qsid, nv));
                            })}
                        />
                        <label>{"V"}</label>
                        <input type="number" value={v.to_string()} min="0" max="255"
                            class="w-16 px-1 py-1 border rounded dark:bg-gray-700 dark:border-gray-600 text-sm"
                            onchange={Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target().unwrap().unchecked_into();
                                let val: u8 = input.value().parse().unwrap_or(0);
                                let mut nv = raw_v.clone();
                                if nv.is_empty() { nv.resize(3, 0); }
                                nv[2] = val;
                                on_change_v.emit((qsid, nv));
                            })}
                        />
                    </div>
                </div>
            }
        }
    }
}
