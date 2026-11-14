//! Svalboard settings panel for SVIL boards, rendered from VIA3 custom UI
//! `menus` combined with `POINTING_HARDWARE` (`0x2C`) and feature flags so
//! options match Keybard's Svalboard Settings UI.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use vial_protocol::svil::diagnostics::{PointingHardware, PointingModule, Side, FLAG4_TRACKPOINT_SPEED};
use vial_protocol::svil::keycode_names::KeycodeNames;
use vial_protocol::svil::menus::{self, CustomValueEntry, ValueRef};

#[derive(Properties, PartialEq)]
pub struct SvalSettingsProps {
    pub menus: Value,
    pub values: Vec<CustomValueEntry>,
    pub names: KeycodeNames,
    pub pointing_hardware: Option<PointingHardware>,
    pub feature_flags3: u8,
    pub feature_flags4: u8,
    pub layer_names: Vec<String>,
    /// One or more writes (for example, resolving both scan waits together).
    pub on_change: Callback<Vec<(ValueRef, Vec<u8>)>>,
}

const SCROLL_SPEED_STEPS: [f64; 20] = [
    0.2, 0.25, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0,
    1.2, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0,
];

const TURBO_US: [u32; 7] = [90, 60, 45, 30, 25, 20, 15];

const SCAN_PERIOD_PRESETS_US: [u32; 9] = [0, 20000, 8000, 4000, 2000, 1000, 500, 250, 125];

const IDLE_STAGE_ORDER: [&str; 4] = [
    "id_scan_idle_after_ms",
    "id_scan_idle_period_ms",
    "id_scan_deep_after_s",
    "id_scan_deep_period_ms",
];

struct DeveloperCopy {
    label: &'static str,
    description: &'static str,
    display_scale: u32,
}

fn developer_copy(key: &str) -> Option<DeveloperCopy> {
    Some(match key {
        "id_scan_prewait_us" => DeveloperCopy {
            label: "Pre-read wait · us",
            description: "How long to wait after selecting a matrix row before reading columns.",
            display_scale: 1,
        },
        "id_scan_postwait_us" => DeveloperCopy {
            label: "Post-read wait · us",
            description: "Settling time after reading a matrix row before driving the next.",
            display_scale: 1,
        },
        "id_scan_period_us" => DeveloperCopy {
            label: "Scan rate",
            description: "Target matrix scan frequency, or Unlimited to scan as fast as possible.",
            display_scale: 1,
        },
        "id_scan_idle_after_ms" => DeveloperCopy {
            label: "Enter idle after · ms",
            description: "Time without key or pointer activity before dropping to the idle scan rate.",
            display_scale: 1,
        },
        "id_scan_idle_period_ms" => DeveloperCopy {
            label: "Idle scan period · ms",
            description: "Time between matrix scans while in normal idle.",
            display_scale: 1,
        },
        "id_scan_deep_after_s" => DeveloperCopy {
            label: "Enter deep idle after · ms",
            description: "Time in idle before dropping to the deep-idle scan rate.",
            display_scale: 1000,
        },
        "id_scan_deep_period_ms" => DeveloperCopy {
            label: "Deep idle scan period · ms",
            description: "Time between matrix scans while in deep idle.",
            display_scale: 1,
        },
        "id_idle_pointer_rest" => DeveloperCopy {
            label: "Allow pointer rest mode",
            description: "Let pointing sensors enter low-power rest when stationary.",
            display_scale: 1,
        },
        "id_idle_rgb_dim" => DeveloperCopy {
            label: "Dim RGB when idle",
            description: "Automatically dim LEDs during idle.",
            display_scale: 1,
        },
        "id_idle_cpu_sleep" => DeveloperCopy {
            label: "CPU sleep between scans",
            description: "Let the MCU sleep between scheduled matrix scans.",
            display_scale: 1,
        },
        "id_idle_low_clock" => DeveloperCopy {
            label: "Drop system clock in deep idle",
            description: "Reduce MCU clock speed during deep idle to save power.",
            display_scale: 1,
        },
        "id_idle_long_nap" => DeveloperCopy {
            label: "Long nap mode",
            description: "Pause between deep-idle wakeups using hardware timer alarms.",
            display_scale: 1,
        },
        "id_scan_deep_clock_idx" => DeveloperCopy {
            label: "Deep idle clock",
            description: "System clock frequency used during deep idle.",
            display_scale: 1,
        },
        _ => return None,
    })
}

/// Convert QMK 8-bit HSV `(0..=255)` to `(r, g, b)`.
pub fn qmk_hsv_to_rgb(h: u8, s: u8, v: u8) -> (u8, u8, u8) {
    if s == 0 {
        return (v, v, v);
    }
    let hue = (h as u32) * 6;
    let region = hue / 255;
    let remainder = ((hue % 255) * 255) / 255;
    let s = s as u32;
    let v = v as u32;
    let p = (v * (255 - s)) / 255;
    let q = (v * (255 - (s * remainder) / 255)) / 255;
    let t = (v * (255 - (s * (255 - remainder)) / 255)) / 255;
    let (r, g, b) = match region {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (r as u8, g as u8, b as u8)
}

/// Map firmware scroll percentage (`20..=1000`) to nearest index in [`SCROLL_SPEED_STEPS`].
pub fn scroll_percent_to_step_index(percent: u32) -> usize {
    let mult = (percent as f64) / 100.0;
    SCROLL_SPEED_STEPS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (*a - mult).abs().partial_cmp(&(*b - mult).abs()).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(9)
}

pub fn step_index_to_scroll_percent(idx: usize) -> u32 {
    let mult = SCROLL_SPEED_STEPS.get(idx).copied().unwrap_or(1.0);
    (mult * 100.0).round() as u32
}

pub fn format_scroll_mult(percent: u32) -> String {
    let idx = scroll_percent_to_step_index(percent);
    let m = SCROLL_SPEED_STEPS[idx];
    if (m.fract()).abs() < 1e-6 {
        format!("{:.0}×", m)
    } else if ((m * 10.0).fract()).abs() < 1e-6 {
        format!("{:.1}×", m)
    } else {
        format!("{:.2}×", m)
    }
}

/// Maximum selectable DPI for a pointing module, matching Keybard's firmware limits.
pub fn max_dpi_for_module(module: Option<PointingModule>, feature_flags4: u8) -> Option<u32> {
    match module {
        Some(PointingModule::Azoteq) => Some(1200),
        Some(PointingModule::TrackPoint) => {
            Some(if feature_flags4 & FLAG4_TRACKPOINT_SPEED != 0 { 3200 } else { 800 })
        }
        Some(PointingModule::Pmw3360) => Some(12000),
        Some(PointingModule::Pmw3389) => Some(16000),
        None => None,
    }
}

/// Filter `(label, value)` DPI options by module maximum while keeping original firmware values.
pub fn filter_dpi_options<'a>(
    options: &'a [(String, u32)],
    module: Option<PointingModule>,
    feature_flags4: u8,
) -> Vec<(&'a str, u32)> {
    let max_dpi = max_dpi_for_module(module, feature_flags4);
    options
        .iter()
        .filter(|(label, _)| match max_dpi {
            Some(limit) => label.trim().parse::<u32>().map(|dpi| dpi <= limit).unwrap_or(true),
            None => true,
        })
        .map(|(label, val)| (label.as_str(), *val))
        .collect()
}

fn select_value(e: &Event) -> Option<String> {
    let target = e.target()?;
    js_sys::Reflect::get(&target, &"value".into()).ok()?.as_string()
}

fn range_u32(item: &Value) -> (u32, u32, u32) {
    let (min, max) = menus::range(item);
    let step = menus::range_step(item).unwrap_or(1).max(1) as u32;
    (min.max(0) as u32, max.max(0) as u32, step)
}

/// Resolve legacy `id_turbo_scan` index when a wait time is stored as `0`.
pub fn effective_wait_us(values: &HashMap<String, u32>, key: &str) -> u32 {
    let stored = values.get(key).copied().unwrap_or(0);
    if stored > 0 {
        return stored;
    }
    let turbo = values.get("id_turbo_scan").copied().unwrap_or(0) as usize;
    TURBO_US.get(turbo).copied().unwrap_or(TURBO_US[0])
}

pub fn format_scan_period_hz(period_us: u32) -> String {
    if period_us == 0 {
        return "Unlimited".to_string();
    }
    let hz = 1_000_000.0 / (period_us as f64);
    if (hz - hz.round()).abs() < 0.05 {
        format!("{:.0} Hz", hz.round())
    } else {
        format!("{:.2} Hz", hz)
    }
}

pub fn hz_to_period_us(hz: f64, min_us: u32, max_us: u32) -> u32 {
    if !hz.is_finite() || hz <= 0.0 {
        return max_us;
    }
    let us = (1_000_000.0 / hz).round() as u32;
    us.clamp(min_us, max_us)
}

fn find_control<'a>(item: &'a Value, target_key: &str) -> Option<(&'a Value, ValueRef)> {
    if let Some(r) = menus::value_ref(item) {
        if r.key == target_key {
            return Some((item, r));
        }
        return None;
    }
    for child in menus::children(item) {
        if let Some(found) = find_control(child, target_key) {
            return Some(found);
        }
    }
    None
}

fn collect_controls<'a>(item: &'a Value, out: &mut Vec<(&'a Value, ValueRef)>) {
    if let Some(r) = menus::value_ref(item) {
        out.push((item, r));
        return;
    }
    for child in menus::children(item) {
        collect_controls(child, out);
    }
}

fn find_section_by_label<'a>(item: &'a Value, target: &str) -> Option<&'a Value> {
    if menus::label(item).eq_ignore_ascii_case(target) {
        return Some(item);
    }
    for child in menus::children(item) {
        if let Some(found) = find_section_by_label(child, target) {
            return Some(found);
        }
    }
    None
}

fn emit_single(on_change: &Callback<Vec<(ValueRef, Vec<u8>)>>, r: ValueRef, width: usize, val: u32) {
    on_change.emit(vec![(r, menus::int_to_bytes(val, width))]);
}

#[function_component]
pub fn SvalSettingsPanel(props: &SvalSettingsProps) -> Html {
    let mut values: HashMap<String, u32> =
        props.values.iter().map(|e| (e.key.clone(), menus::bytes_to_int(&e.data))).collect();
    let app_pads_os = u32::from(
        props
            .pointing_hardware
            .as_ref()
            .is_some_and(|hw| hw.pads_in_touchpad_mode()),
    );
    values.insert("app_pads_os".to_string(), app_pads_os);

    let top: Vec<&Value> = props
        .menus
        .as_array()
        .map(|a| a.iter().filter(|v| v.is_object()).collect())
        .unwrap_or_default();

    if top.is_empty() {
        return html! { <p class="text-gray-500">{"This keyboard has no Svalboard settings menus."}</p> };
    }

    let mut rendered_sections: Vec<Html> = Vec::new();
    for item in top.into_iter().filter(|i| menus::is_shown(i, &values)) {
        let lbl = menus::label(item);
        let lower = lbl.to_lowercase();
        if lower.contains("pointing") {
            rendered_sections.push(render_pointing_menu(item, &values, props));
            if let Some(adv) = find_section_by_label(item, "Advanced") {
                if menus::is_shown(adv, &values) {
                    rendered_sections.push(render_power_and_scanning(adv, &values, props));
                }
            }
        } else if lbl.eq_ignore_ascii_case("Power and scanning") {
            rendered_sections.push(render_power_and_scanning(item, &values, props));
        } else if lbl.eq_ignore_ascii_case("Layer Colors") {
            rendered_sections.push(render_layer_colors(item, &values, props));
        } else {
            rendered_sections.push(html! {
                <div class="p-5 bg-gray-50 dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700">
                    <h3 class="text-lg font-semibold mb-1">{lbl}</h3>
                    { render_note_and_desc(item) }
                    <div class="mt-3">
                        { render_items(&menus::children(item), &values, props, 0) }
                    </div>
                </div>
            });
        }
    }

    html! {
        <div class="w-full max-w-3xl mx-auto space-y-6">
            <div>
                <h2 class="text-xl font-bold">{"Svalboard Settings"}</h2>
                <p class="text-sm text-gray-500 mt-0.5">
                    {"Changes are written to the keyboard and saved immediately."}
                </p>
            </div>
            { for rendered_sections }
        </div>
    }
}

fn render_note_and_desc(item: &Value) -> Html {
    let note = menus::note(item);
    let desc = menus::description(item);
    if note.is_none() && desc.is_none() {
        return html! {};
    }
    html! {
        <div class="space-y-1 mb-2">
            if let Some(d) = desc {
                <p class="text-xs text-gray-600 dark:text-gray-400">{d}</p>
            }
            if let Some(n) = note {
                <p class="text-xs text-amber-700 dark:text-amber-300 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded px-2.5 py-1.5">
                    {n}
                </p>
            }
        </div>
    }
}

fn render_pointing_menu(menu: &Value, values: &HashMap<String, u32>, props: &SvalSettingsProps) -> Html {
    let left_card = render_hand_card(Side::Left, menu, values, props);
    let right_card = render_hand_card(Side::Right, menu, values, props);

    let scroll_settings = find_section_by_label(menu, "Scroll Settings");
    let touchpad_section = find_section_by_label(menu, "Touchpad");
    let automouse_section = find_section_by_label(menu, "Auto Mouse");

    let board_has_touchpad = props
        .pointing_hardware
        .as_ref()
        .is_none_or(|hw| hw.has_touchpad());

    let handled_labels = [
        "left pointer",
        "right pointer",
        "scroll speed",
        "scroll settings",
        "touchpad",
        "auto mouse",
        "advanced",
    ];
    let extra_sections: Vec<&Value> = menus::children(menu)
        .into_iter()
        .filter(|child| {
            let l = menus::label(child).to_lowercase();
            !handled_labels.contains(&l.as_str()) && menus::is_shown(child, values)
        })
        .collect();

    html! {
        <div class="p-5 bg-gray-50 dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 space-y-5">
            <h3 class="text-lg font-semibold">{menus::label(menu)}</h3>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                { left_card }
                { right_card }
            </div>

            if let Some(sec) = scroll_settings {
                if menus::is_shown(sec, values) {
                    <div class="pt-3 border-t border-gray-200 dark:border-gray-700">
                        <h4 class="text-sm font-semibold uppercase tracking-wide text-gray-500 mb-2">
                            {menus::label(sec)}
                        </h4>
                        { render_note_and_desc(sec) }
                        { render_items(&menus::children(sec), values, props, 1) }
                    </div>
                }
            }

            if board_has_touchpad {
                if let Some(sec) = touchpad_section {
                    if menus::is_shown(sec, values) {
                        <div class="pt-3 border-t border-gray-200 dark:border-gray-700">
                            <h4 class="text-sm font-semibold uppercase tracking-wide text-gray-500 mb-2">
                                {menus::label(sec)}
                            </h4>
                            { render_note_and_desc(sec) }
                            { render_items(&menus::children(sec), values, props, 1) }
                        </div>
                    }
                }
            }

            if let Some(sec) = automouse_section {
                if menus::is_shown(sec, values) {
                    <div class="pt-3 border-t border-gray-200 dark:border-gray-700">
                        <h4 class="text-sm font-semibold uppercase tracking-wide text-gray-500 mb-2">
                            {menus::label(sec)}
                        </h4>
                        { render_note_and_desc(sec) }
                        { render_items(&menus::children(sec), values, props, 1) }
                    </div>
                }
            }

            if !extra_sections.is_empty() {
                <div class="pt-3 border-t border-gray-200 dark:border-gray-700">
                    { render_items(&extra_sections, values, props, 1) }
                </div>
            }
        </div>
    }
}

fn hand_hardware_badge(hw: Option<&PointingHardware>, side: Side) -> (Option<PointingModule>, bool, Option<String>) {
    let Some(half) = hw.and_then(|h| h.halves.iter().find(|x| x.side == side)) else {
        return (None, false, None);
    };
    let module = half.effective_module();
    let label = match module {
        Some(m) => match m {
            PointingModule::Pmw3360 => Some("Trackball · PMW3360".to_string()),
            PointingModule::Pmw3389 => Some("Trackball · PMW3389".to_string()),
            PointingModule::Azoteq => Some("Touchpad · Azoteq".to_string()),
            PointingModule::TrackPoint => Some("TrackPoint".to_string()),
        },
        None => Some("No pointing device detected".to_string()),
    };
    (module, half.os_touchpad, label)
}

fn render_hand_card(
    side: Side,
    menu: &Value,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let (title, dpi_key, scroll_key, scroll_speed_key) = match side {
        Side::Left => ("Left Hand", "id_left_dpi", "id_left_scroll", "id_left_scroll_speed"),
        Side::Right => ("Right Hand", "id_right_dpi", "id_right_scroll", "id_right_scroll_speed"),
    };
    let (module, os_touchpad, module_badge) =
        hand_hardware_badge(props.pointing_hardware.as_ref(), side);

    let dpi_ctrl = find_control(menu, dpi_key);
    let scroll_speed_ctrl = find_control(menu, scroll_speed_key);
    let scroll_ctrl = find_control(menu, scroll_key);

    html! {
        <div class="p-4 rounded-lg bg-white dark:bg-gray-900/60 border border-gray-200 dark:border-gray-700 space-y-3">
            <div class="flex items-center justify-between gap-2">
                <h4 class="font-semibold text-base">{title}</h4>
                if let Some(badge) = module_badge {
                    <span class="text-xs px-2 py-0.5 rounded-full bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 border border-gray-200 dark:border-gray-700">
                        {badge}
                    </span>
                }
            </div>

            if let Some((item, r)) = dpi_ctrl {
                if menus::is_shown(item, values) {
                    { render_dpi_select(item, r, module, os_touchpad, values, props) }
                }
            }

            if let Some((item, r)) = scroll_speed_ctrl {
                if menus::is_shown(item, values) {
                    { render_scroll_speed(item, r, os_touchpad, values, props) }
                }
            }

            if let Some((item, r)) = scroll_ctrl {
                if menus::is_shown(item, values) {
                    { render_hand_toggle(item, r, os_touchpad, values, props) }
                }
            }

            if os_touchpad {
                <p class="text-xs text-gray-500">
                    {"Controlled by the OS (Precision Touchpad mode)."}
                </p>
            }
        </div>
    }
}

fn render_dpi_select(
    item: &Value,
    r: ValueRef,
    module: Option<PointingModule>,
    os_touchpad: bool,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let options = menus::dropdown_options(item);
    let filtered = filter_dpi_options(&options, module, props.feature_flags4);
    let current = values.get(&r.key).copied().unwrap_or(0);
    let disabled = os_touchpad || menus::is_disabled(item, values);
    let width = menus::byte_width(item);
    let on_change = props.on_change.clone();

    let onchange = Callback::from(move |e: Event| {
        if let Some(val_str) = select_value(&e) {
            let v: u32 = val_str.parse().unwrap_or(0);
            emit_single(&on_change, r.clone(), width, v);
        }
    });

    html! {
        <div class={classes!(
            "flex", "items-center", "justify-between", "gap-2",
            disabled.then_some("opacity-50")
        )}>
            <span class="text-sm">{"Pointer speed"}</span>
            <select
                class="px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 text-sm"
                disabled={disabled}
                onchange={onchange}
            >
                { for filtered.into_iter().map(|(text, val)| html! {
                    <option value={val.to_string()} selected={val == current}>{text}</option>
                })}
            </select>
        </div>
    }
}

fn render_scroll_speed(
    item: &Value,
    r: ValueRef,
    os_touchpad: bool,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let current_pct = values.get(&r.key).copied().unwrap_or(100);
    let step_idx = scroll_percent_to_step_index(current_pct);
    let disabled = os_touchpad || menus::is_disabled(item, values);
    let width = menus::byte_width(item);
    let on_change = props.on_change.clone();

    let onchange = Callback::from(move |e: Event| {
        if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            let idx = i.value().parse::<usize>().unwrap_or(9);
            let pct = step_index_to_scroll_percent(idx);
            emit_single(&on_change, r.clone(), width, pct);
        }
    });

    html! {
        <div class={classes!(
            "flex", "items-center", "justify-between", "gap-2",
            disabled.then_some("opacity-50")
        )}>
            <span class="text-sm">{"Scroll speed"}</span>
            <div class="flex items-center gap-2">
                <input
                    type="range"
                    min="0"
                    max={(SCROLL_SPEED_STEPS.len() - 1).to_string()}
                    step="1"
                    value={step_idx.to_string()}
                    disabled={disabled}
                    class="w-28"
                    onchange={onchange}
                />
                <span class="w-12 text-right font-mono text-xs">{format_scroll_mult(current_pct)}</span>
            </div>
        </div>
    }
}

fn render_hand_toggle(
    item: &Value,
    r: ValueRef,
    os_touchpad: bool,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let (off_val, on_val) = menus::toggle_values(item);
    let current = values.get(&r.key).copied().unwrap_or(0);
    let checked = current == on_val;
    let disabled = os_touchpad || menus::is_disabled(item, values);
    let width = menus::byte_width(item);
    let on_change = props.on_change.clone();

    let onchange = Callback::from(move |e: Event| {
        if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            emit_single(&on_change, r.clone(), width, if i.checked() { on_val } else { off_val });
        }
    });

    html! {
        <div class={classes!(
            "flex", "items-center", "justify-between", "gap-2",
            disabled.then_some("opacity-50")
        )}>
            <span class="text-sm">{menus::label(item)}</span>
            <input type="checkbox" checked={checked} disabled={disabled} class="h-4 w-4 accent-blue-600" onchange={onchange} />
        </div>
    }
}

fn render_power_and_scanning(
    menu: &Value,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let mut raw_controls: Vec<(&Value, ValueRef)> = Vec::new();
    collect_controls(menu, &mut raw_controls);

    let mut by_key: HashMap<String, (&Value, ValueRef)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (item, r) in raw_controls {
        if r.key == "id_turbo_scan" || menus::is_link(item) || !menus::is_shown(item, values) {
            continue;
        }
        if !by_key.contains_key(&r.key) {
            order.push(r.key.clone());
            by_key.insert(r.key.clone(), (item, r));
        }
    }

    let mut priority_keys: Vec<String> = Vec::new();
    for fixed in [
        "id_scan_period_us",
        "id_idle_cpu_sleep",
        "id_scan_prewait_us",
        "id_scan_postwait_us",
    ] {
        if by_key.contains_key(fixed) {
            priority_keys.push(fixed.to_string());
        }
    }
    for stage in IDLE_STAGE_ORDER {
        if by_key.contains_key(stage) && !priority_keys.iter().any(|k| k == stage) {
            priority_keys.push(stage.to_string());
        }
    }
    let seen: HashSet<String> = priority_keys.iter().cloned().collect();
    for key in order {
        if !seen.contains(&key) {
            priority_keys.push(key);
        }
    }

    let prewait_pair = by_key.get("id_scan_prewait_us").map(|(i, r)| (*i, r.clone()));
    let postwait_pair = by_key.get("id_scan_postwait_us").map(|(i, r)| (*i, r.clone()));

    html! {
        <div class="p-5 bg-gray-50 dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 space-y-3">
            <div>
                <h3 class="text-lg font-semibold">{"Power and scanning"}</h3>
                { render_note_and_desc(menu) }
            </div>
            <div class="space-y-1">
                { for priority_keys.into_iter().filter_map(|key| {
                    let (item, r) = by_key.get(&key)?.clone();
                    Some(if key == "id_scan_period_us" {
                        render_scan_rate_control(item, r, values, props)
                    } else if key == "id_scan_prewait_us" || key == "id_scan_postwait_us" {
                        let sibling = if key == "id_scan_prewait_us" {
                            postwait_pair.clone()
                        } else {
                            prewait_pair.clone()
                        };
                        render_scan_wait_control(item, r, sibling, values, props)
                    } else {
                        render_dev_control(item, r, values, props)
                    })
                })}
            </div>
        </div>
    }
}

fn render_scan_rate_control(
    item: &Value,
    r: ValueRef,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let copy = developer_copy(&r.key);
    let label = copy.as_ref().map(|c| c.label).unwrap_or_else(|| menus::label(item));
    let desc = copy.as_ref().map(|c| c.description);
    let (_, max_us, _) = range_u32(item);
    let max_us = max_us.max(1);
    let min_us = 1u32;
    let current = values.get(&r.key).copied().unwrap_or(0);
    let is_preset = SCAN_PERIOD_PRESETS_US.contains(&current);
    let disabled = menus::is_disabled(item, values);
    let width = menus::byte_width(item);

    let on_select = {
        let on_change = props.on_change.clone();
        let r = r.clone();
        Callback::from(move |e: Event| {
            if let Some(val_str) = select_value(&e) {
                if val_str == "custom" {
                    let fallback = if current > 0 { current } else { 1000 };
                    emit_single(&on_change, r.clone(), width, fallback.clamp(min_us, max_us));
                } else if let Ok(v) = val_str.parse::<u32>() {
                    emit_single(&on_change, r.clone(), width, v);
                }
            }
        })
    };

    let on_custom_hz = {
        let on_change = props.on_change.clone();
        let r = r.clone();
        Callback::from(move |e: Event| {
            if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                if let Ok(hz) = i.value().parse::<f64>() {
                    let period = hz_to_period_us(hz, min_us, max_us);
                    emit_single(&on_change, r.clone(), width, period);
                }
            }
        })
    };

    let current_hz = if current > 0 {
        format!("{:.0}", (1_000_000.0 / current as f64).round())
    } else {
        "1000".to_string()
    };

    html! {
        <div class={classes!(
            "py-2", "flex", "items-start", "justify-between", "gap-4", "border-b",
            "border-gray-200/70", "dark:border-gray-700/70", "last:border-b-0",
            disabled.then_some("opacity-50")
        )}>
            <div class="min-w-0">
                <div class="text-sm font-medium">{label}</div>
                if let Some(d) = desc {
                    <div class="text-xs text-gray-500 dark:text-gray-400">{d}</div>
                }
            </div>
            <div class="flex items-center gap-2 shrink-0">
                <select
                    class="px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900 text-sm"
                    disabled={disabled}
                    onchange={on_select}
                >
                    { for SCAN_PERIOD_PRESETS_US.into_iter().map(|p| html! {
                        <option value={p.to_string()} selected={current == p}>{format_scan_period_hz(p)}</option>
                    })}
                    <option value="custom" selected={!is_preset}>{"Custom rate…"}</option>
                </select>
                if !is_preset {
                    <div class="flex items-center gap-1">
                        <input
                            type="number"
                            min="1"
                            max="10000"
                            value={current_hz}
                            disabled={disabled}
                            class="w-20 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900 text-sm font-mono"
                            onchange={on_custom_hz}
                        />
                        <span class="text-xs text-gray-500">{"Hz"}</span>
                    </div>
                }
            </div>
        </div>
    }
}

fn render_scan_wait_control(
    item: &Value,
    r: ValueRef,
    sibling: Option<(&Value, ValueRef)>,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let copy = developer_copy(&r.key);
    let label = copy.as_ref().map(|c| c.label).unwrap_or_else(|| menus::label(item));
    let desc = copy.as_ref().map(|c| c.description);
    let (raw_min, raw_max, step) = range_u32(item);
    let min = raw_min.max(1);
    let max = raw_max.max(min);
    let width = menus::byte_width(item);
    let current = effective_wait_us(values, &r.key).clamp(min, max);
    let disabled = menus::is_disabled(item, values);

    let sibling_write_info = sibling.and_then(|(sib_item, sib_ref)| {
        let sib_stored = values.get(&sib_ref.key).copied().unwrap_or(0);
        if sib_stored == 0 {
            let (s_min, s_max, _) = range_u32(sib_item);
            let s_min = s_min.max(1);
            let s_max = s_max.max(s_min);
            let sib_eff = effective_wait_us(values, &sib_ref.key).clamp(s_min, s_max);
            Some((sib_ref, menus::byte_width(sib_item), sib_eff))
        } else {
            None
        }
    });

    let on_change = props.on_change.clone();
    let onchange = Callback::from(move |e: Event| {
        if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            let v = i.value().parse::<u32>().unwrap_or(min).clamp(min, max);
            let mut batch = vec![(r.clone(), menus::int_to_bytes(v, width))];
            if let Some((sib_ref, sib_width, sib_val)) = sibling_write_info.clone() {
                batch.push((sib_ref, menus::int_to_bytes(sib_val, sib_width)));
            }
            on_change.emit(batch);
        }
    });

    html! {
        <div class={classes!(
            "py-2", "flex", "items-start", "justify-between", "gap-4", "border-b",
            "border-gray-200/70", "dark:border-gray-700/70", "last:border-b-0",
            disabled.then_some("opacity-50")
        )}>
            <div class="min-w-0">
                <div class="text-sm font-medium">{label}</div>
                if let Some(d) = desc {
                    <div class="text-xs text-gray-500 dark:text-gray-400">{d}</div>
                }
            </div>
            <div class="flex items-center gap-3 shrink-0">
                <input
                    type="range"
                    min={min.to_string()}
                    max={max.to_string()}
                    step={step.to_string()}
                    value={current.to_string()}
                    disabled={disabled}
                    class="w-40"
                    onchange={onchange}
                />
                <span class="w-16 text-right font-mono text-xs">{format!("{current} us")}</span>
            </div>
        </div>
    }
}

fn render_dev_control(
    item: &Value,
    r: ValueRef,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let copy = developer_copy(&r.key);
    let label_override = copy.as_ref().map(|c| c.label);
    let desc_override = copy.as_ref().map(|c| c.description);
    let display_scale = copy.as_ref().map(|c| c.display_scale).unwrap_or(1);

    render_control_with_overrides(item, r, values, props, label_override, desc_override, display_scale, 0)
}

fn render_layer_colors(
    menu: &Value,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let controls: Vec<(&Value, ValueRef)> = {
        let mut out = Vec::new();
        collect_controls(menu, &mut out);
        out
    };

    html! {
        <div class="p-5 bg-gray-50 dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 space-y-3">
            <div>
                <h3 class="text-lg font-semibold">{menus::label(menu)}</h3>
                { render_note_and_desc(menu) }
            </div>
            <div class="space-y-2">
                { for controls.into_iter().enumerate().filter(|( _, (item, _) )| menus::is_shown(item, values)).map(|(fallback_idx, (item, r))| {
                    let layer_idx = r.key
                        .strip_prefix("id_layer")
                        .and_then(|s| s.strip_suffix("_color"))
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or(fallback_idx);
                    let layer_name = props
                        .layer_names
                        .get(layer_idx)
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());
                    let label = match layer_name {
                        Some(name) => format!("Layer {layer_idx}: {name}"),
                        None => menus::label(item).to_string(),
                    };
                    render_color_control(item, r, &label, values, props)
                })}
            </div>
        </div>
    }
}

fn render_color_control(
    item: &Value,
    r: ValueRef,
    label: &str,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
) -> Html {
    let current = values.get(&r.key).copied().unwrap_or(0);
    let hue = (current & 0xFF) as u8;
    let sat = ((current >> 8) & 0xFF) as u8;
    let (cr, cg, cb) = qmk_hsv_to_rgb(hue, sat, 255);
    let swatch = format!(
        "background: rgb({cr}, {cg}, {cb}); width: 1.5rem; height: 1.5rem; border-radius: 0.375rem; border: 1px solid rgba(128,128,128,0.4);"
    );
    let disabled = menus::is_disabled(item, values);
    let width = menus::byte_width(item);

    let on_hue = {
        let on_change = props.on_change.clone();
        let r = r.clone();
        Callback::from(move |e: Event| {
            if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                let h = i.value().parse::<u32>().unwrap_or(0).min(255);
                emit_single(&on_change, r.clone(), width, h | ((sat as u32) << 8));
            }
        })
    };
    let on_sat = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: Event| {
            if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                let s = i.value().parse::<u32>().unwrap_or(0).min(255);
                emit_single(&on_change, r.clone(), width, (hue as u32) | (s << 8));
            }
        })
    };

    html! {
        <div class={classes!(
            "py-1.5", "flex", "items-center", "justify-between", "gap-4",
            disabled.then_some("opacity-50")
        )}>
            <span class="text-sm">{label.to_string()}</span>
            <div class="flex items-center gap-2 text-sm">
                <span class="text-xs text-gray-500">{"H"}</span>
                <input type="range" min="0" max="255" value={hue.to_string()} disabled={disabled} class="w-28" onchange={on_hue} />
                <span class="text-xs text-gray-500">{"S"}</span>
                <input type="range" min="0" max="255" value={sat.to_string()} disabled={disabled} class="w-28" onchange={on_sat} />
                <div style={swatch} title={format!("HSV ({hue}, {sat}, 255) -> rgb({cr}, {cg}, {cb})")}></div>
            </div>
        </div>
    }
}

fn render_items(items: &[&Value], values: &HashMap<String, u32>, props: &SvalSettingsProps, depth: usize) -> Html {
    html! {
        <div class={if depth > 0 { "space-y-2 mt-2 pl-4 border-l-2 border-gray-200 dark:border-gray-700" } else { "space-y-3" }}>
            { for items.iter().filter(|i| !menus::is_link(i) && menus::is_shown(i, values)).map(|item| {
                if let Some(r) = menus::value_ref(item) {
                    render_control(item, r, values, props)
                } else {
                    let lbl = menus::label(item);
                    let children = menus::children(item);
                    html! {
                        <div>
                            if !lbl.is_empty() {
                                <h4 class="text-sm font-medium text-gray-600 dark:text-gray-300 mt-3 mb-1">{lbl}</h4>
                            }
                            { render_note_and_desc(item) }
                            { render_items(&children, values, props, depth + 1) }
                        </div>
                    }
                }
            })}
        </div>
    }
}

fn format_range_value(key: &str, displayed: u32) -> String {
    if matches!(key, "id_pad_dead_zone" | "id_pad_scroll_start") {
        return format!("{:.2} mm", (displayed as f64) / 100.0);
    }
    if key == "id_automouse_decay" {
        return format!("{} ms", displayed * 10);
    }
    displayed.to_string()
}

fn render_control(item: &Value, r: ValueRef, values: &HashMap<String, u32>, props: &SvalSettingsProps) -> Html {
    render_control_with_overrides(item, r, values, props, None, None, 1, 0)
}

#[allow(clippy::too_many_arguments)]
fn render_control_with_overrides(
    item: &Value,
    r: ValueRef,
    values: &HashMap<String, u32>,
    props: &SvalSettingsProps,
    label_override: Option<&str>,
    desc_override: Option<&str>,
    display_scale: u32,
    _depth: usize,
) -> Html {
    let current = values.get(&r.key).copied().unwrap_or(0);
    let width = menus::byte_width(item);
    let label = label_override.unwrap_or_else(|| menus::label(item)).to_string();
    let desc = desc_override.or_else(|| menus::description(item)).map(|s| s.to_string());
    let disabled = menus::is_disabled(item, values);
    let key = r.key.clone();

    let emit = {
        let on_change = props.on_change.clone();
        let r = r.clone();
        move |v: u32| emit_single(&on_change, r.clone(), width, v)
    };

    let row = |control: Html| {
        html! {
            <div class={classes!(
                "py-1.5", "flex", "items-start", "justify-between", "gap-4",
                disabled.then_some("opacity-50")
            )}>
                <div class="min-w-0">
                    <div class="text-sm">{label.clone()}</div>
                    if let Some(d) = desc.clone() {
                        <div class="text-xs text-gray-500 dark:text-gray-400">{d}</div>
                    }
                </div>
                <div class="flex items-center gap-3 text-sm shrink-0">{control}</div>
            </div>
        }
    };

    match menus::item_type(item) {
        "range" => {
            let (raw_min, raw_max, raw_step) = range_u32(item);
            let scale = display_scale.max(1);
            let d_min = raw_min.saturating_mul(scale);
            let d_max = raw_max.saturating_mul(scale);
            let d_step = raw_step.saturating_mul(scale);
            let d_current = current.saturating_mul(scale).clamp(d_min, d_max);

            let onchange = Callback::from(move |e: Event| {
                if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                    let entered = i.value().parse::<u32>().unwrap_or(d_min).clamp(d_min, d_max);
                    let raw = if scale > 1 {
                        ((entered + scale / 2) / scale).clamp(raw_min, raw_max)
                    } else {
                        entered
                    };
                    emit(raw);
                }
            });
            row(html! {
                <>
                    <input
                        type="range"
                        min={d_min.to_string()}
                        max={d_max.to_string()}
                        step={d_step.to_string()}
                        value={d_current.to_string()}
                        disabled={disabled}
                        class="w-40"
                        onchange={onchange}
                    />
                    <span class="w-20 text-right font-mono text-xs">{format_range_value(&key, d_current)}</span>
                </>
            })
        }
        "toggle" => {
            let (off_val, on_val) = menus::toggle_values(item);
            let checked = current == on_val;
            let onchange = Callback::from(move |e: Event| {
                if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                    emit(if i.checked() { on_val } else { off_val });
                }
            });
            row(html! {
                <input type="checkbox" checked={checked} disabled={disabled} class="h-4 w-4 accent-blue-600" onchange={onchange} />
            })
        }
        "dropdown" => {
            let options = menus::dropdown_options(item);
            let onchange = Callback::from(move |e: Event| {
                if let Some(val_str) = select_value(&e) {
                    emit(val_str.parse().unwrap_or(0));
                }
            });
            row(html! {
                <select
                    class="px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900"
                    disabled={disabled}
                    onchange={onchange}
                >
                    { for options.into_iter().map(|(text, val)| html! {
                        <option value={val.to_string()} selected={val == current}>{text}</option>
                    })}
                </select>
            })
        }
        "color" => render_color_control(item, r, &label, values, props),
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
                <input
                    type="text"
                    value={text}
                    disabled={disabled}
                    class="w-48 px-2 py-1 font-mono rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-900"
                    onchange={onchange}
                />
            })
        }
        "button" => {
            let onclick = Callback::from(move |_: MouseEvent| emit(1));
            row(html! {
                <button
                    class="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white rounded"
                    disabled={disabled}
                    onclick={onclick}
                >
                    {"Run"}
                </button>
            })
        }
        other => row(html! { <span class="text-gray-400 font-mono">{format!("{other}: {current}")}</span> }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qmk_hsv_to_rgb_white_red_and_saturation() {
        assert_eq!(qmk_hsv_to_rgb(0, 0, 255), (255, 255, 255));
        assert_eq!(qmk_hsv_to_rgb(0, 255, 255), (255, 0, 0));
        assert_eq!(qmk_hsv_to_rgb(85, 255, 255), (0, 255, 0));
        assert_eq!(qmk_hsv_to_rgb(170, 255, 255), (0, 0, 255));
    }

    #[test]
    fn scroll_speed_roundtrip_and_formatting() {
        assert_eq!(step_index_to_scroll_percent(0), 20);
        assert_eq!(step_index_to_scroll_percent(9), 100);
        assert_eq!(step_index_to_scroll_percent(19), 1000);
        assert_eq!(scroll_percent_to_step_index(100), 9);
        assert_eq!(scroll_percent_to_step_index(25), 1);
        assert_eq!(format_scroll_mult(100), "1×");
        assert_eq!(format_scroll_mult(25), "0.25×");
    }

    #[test]
    fn dpi_filtering_caps_by_module_and_preserves_indices() {
        let opts: Vec<(String, u32)> = ["200", "400", "800", "1200", "1600", "3200", "12000", "16000"]
            .into_iter()
            .enumerate()
            .map(|(i, s)| (s.to_string(), i as u32))
            .collect();
        assert_eq!(
            filter_dpi_options(&opts, Some(PointingModule::Azoteq), 0),
            vec![("200", 0), ("400", 1), ("800", 2), ("1200", 3)]
        );
        assert_eq!(
            filter_dpi_options(&opts, Some(PointingModule::TrackPoint), 0),
            vec![("200", 0), ("400", 1), ("800", 2)]
        );
        assert_eq!(
            filter_dpi_options(&opts, Some(PointingModule::TrackPoint), FLAG4_TRACKPOINT_SPEED),
            vec![("200", 0), ("400", 1), ("800", 2), ("1200", 3), ("1600", 4), ("3200", 5)]
        );
        assert_eq!(
            filter_dpi_options(&opts, Some(PointingModule::Pmw3360), 0).last(),
            Some(&("12000", 6))
        );
    }

    #[test]
    fn scan_wait_fallback_and_hz_conversion() {
        let mut map = HashMap::new();
        map.insert("id_turbo_scan".to_string(), 2);
        map.insert("id_scan_prewait_us".to_string(), 0);
        map.insert("id_scan_postwait_us".to_string(), 12);
        assert_eq!(effective_wait_us(&map, "id_scan_prewait_us"), 45);
        assert_eq!(effective_wait_us(&map, "id_scan_postwait_us"), 12);

        assert_eq!(format_scan_period_hz(0), "Unlimited");
        assert_eq!(format_scan_period_hz(1000), "1000 Hz");
        assert_eq!(hz_to_period_us(500.0, 1, 50000), 2000);
    }
}
