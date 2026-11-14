//! VIA3 custom UI menus (the `menus` of a keyboard definition), which SVIL
//! boards use for their own settings (pointing devices, layer colors, ...).
//!
//! A menu item is either a section `{label, content: [items...], showIf?}` or a
//! control `{label, type, options?, content: ["id_key", channel, value_id], showIf?}`.
//! Values are read and written with the VIA custom value commands.

use super::diagnostics::{FLAG3_PAD_GESTURES, FLAG3_PAD_TUNING, FLAG4_PAD_MODE, FLAG4_PAD_PALM, FLAG4_SCROLL_SPEED};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Where a control's value lives.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValueRef {
    pub key: String,
    pub channel: u8,
    pub value_id: u8,
}

/// A stored custom value, as in `.svil` `custom_values`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomValueEntry {
    pub key: String,
    pub channel: u8,
    pub value_id: u8,
    pub data: Vec<u8>,
}

/// The value reference of a control item, `None` for a section.
pub fn value_ref(item: &Value) -> Option<ValueRef> {
    let content = item.get("content")?.as_array()?;
    let key = content.first()?.as_str()?;
    if content.len() < 3 {
        return None;
    }
    Some(ValueRef {
        key: key.to_string(),
        channel: content[1].as_u64()? as u8,
        value_id: content[2].as_u64()? as u8,
    })
}

/// The children of a section (empty for a control).
pub fn children(item: &Value) -> Vec<&Value> {
    match item.get("content").and_then(|c| c.as_array()) {
        Some(arr) if arr.first().is_some_and(|f| f.is_object()) => arr.iter().filter(|v| v.is_object()).collect(),
        _ => Vec::new(),
    }
}

pub fn item_type(item: &Value) -> &str {
    item.get("type").and_then(|t| t.as_str()).unwrap_or("")
}

pub fn label(item: &Value) -> &str {
    item.get("label").and_then(|t| t.as_str()).unwrap_or("")
}

pub fn note(item: &Value) -> Option<&str> {
    item.get("note").and_then(|t| t.as_str()).filter(|s| !s.is_empty())
}

pub fn description(item: &Value) -> Option<&str> {
    item.get("description").and_then(|t| t.as_str()).filter(|s| !s.is_empty())
}

pub fn is_link(item: &Value) -> bool {
    item_type(item) == "link"
}

/// `[min, max]` of a range control (default 0..255).
pub fn range(item: &Value) -> (i64, i64) {
    let opts = item.get("options").and_then(|o| o.as_array());
    let get = |i: usize, d: i64| opts.and_then(|o| o.get(i)).and_then(|v| v.as_i64()).unwrap_or(d);
    (get(0, 0), get(1, 255))
}

/// Optional step (`options[2]`) of a range control.
pub fn range_step(item: &Value) -> Option<i64> {
    item.get("options")
        .and_then(|o| o.as_array())
        .and_then(|o| o.get(2))
        .and_then(|v| v.as_i64())
        .filter(|&s| s > 0)
}

/// Dropdown choices as `(label, value)`: options are labels (value = position)
/// or `[label, value]` pairs.
pub fn dropdown_options(item: &Value) -> Vec<(String, u32)> {
    let Some(opts) = item.get("options").and_then(|o| o.as_array()) else { return Vec::new() };
    opts.iter()
        .enumerate()
        .filter_map(|(i, o)| match o {
            Value::String(s) => Some((s.clone(), i as u32)),
            Value::Array(pair) => {
                let l = pair.first()?.as_str()?.to_string();
                let v = pair.get(1).and_then(|v| v.as_u64()).unwrap_or(i as u64) as u32;
                Some((l, v))
            }
            _ => None,
        })
        .collect()
}

/// Toggle on/off values: `options: [off, on]`, default `[0, 1]`.
pub fn toggle_values(item: &Value) -> (u32, u32) {
    let opts = item.get("options").and_then(|o| o.as_array());
    let get = |i: usize, d: u32| opts.and_then(|o| o.get(i)).and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(d);
    (get(0, 0), get(1, 1))
}

/// Byte width of a control's value.
pub fn byte_width(item: &Value) -> usize {
    match item_type(item) {
        "color" | "keycode" => 2,
        "range" => {
            if range(item).1 > 255 {
                2
            } else {
                1
            }
        }
        _ => 1,
    }
}

/// Every control with a value in the menu tree, depth first.
pub fn controls(menus: &Value) -> Vec<(&Value, ValueRef)> {
    fn walk<'a>(items: &[&'a Value], out: &mut Vec<(&'a Value, ValueRef)>) {
        for item in items {
            if let Some(r) = value_ref(item) {
                out.push((item, r));
            } else {
                walk(&children(item), out);
            }
        }
    }
    let mut out = Vec::new();
    let top: Vec<&Value> = menus.as_array().map(|a| a.iter().filter(|v| v.is_object()).collect()).unwrap_or_default();
    walk(&top, &mut out);
    out
}

/// Little-endian bytes to integer.
pub fn bytes_to_int(data: &[u8]) -> u32 {
    data.iter().take(4).enumerate().fold(0u32, |acc, (i, b)| acc | (*b as u32) << (8 * i))
}

/// Integer to `width` little-endian bytes.
pub fn int_to_bytes(value: u32, width: usize) -> Vec<u8> {
    (0..width).map(|i| (value >> (8 * i)) as u8).collect()
}

/// Evaluate a `showIf` condition such as `{id_automouse_enable} == 1`.
///
/// `None` when the value it depends on is unknown; `Some(true)` for a missing or
/// malformed expression (a definition problem, not board state).
pub fn evaluate_condition(expr: &str, values: &HashMap<String, u32>) -> Option<bool> {
    let e = expr.trim();
    if e.is_empty() {
        return Some(true);
    }
    let parse = || -> Option<(String, &str, i64)> {
        let rest = e.strip_prefix('{')?;
        let (key, rest) = rest.split_once('}')?;
        let rest = rest.trim_start();
        let op = ["==", "!=", "<=", ">=", "<", ">"].into_iter().find(|op| rest.starts_with(op))?;
        let num: i64 = rest[op.len()..].trim().parse().ok()?;
        Some((key.trim().to_string(), op, num))
    };
    let Some((key, op, num)) = parse() else { return Some(true) };
    let v = *values.get(&key)? as i64;
    Some(match op {
        "==" => v == num,
        "!=" => v != num,
        "<=" => v <= num,
        ">=" => v >= num,
        "<" => v < num,
        _ => v > num,
    })
}

/// Whether an item is shown (hidden while the value it depends on is unknown).
pub fn is_shown(item: &Value, values: &HashMap<String, u32>) -> bool {
    match item.get("showIf").and_then(|s| s.as_str()) {
        Some(expr) => evaluate_condition(expr, values).unwrap_or(false),
        None => true,
    }
}

/// Whether an item is disabled via `disabledIf` (false while unknown).
pub fn is_disabled(item: &Value, values: &HashMap<String, u32>) -> bool {
    match item.get("disabledIf").and_then(|s| s.as_str()) {
        Some(expr) => evaluate_condition(expr, values).unwrap_or(false),
        None => false,
    }
}

fn has_control_key(item: &Value, keys: &[&str]) -> bool {
    if let Some(r) = value_ref(item) {
        return keys.contains(&r.key.as_str());
    }
    children(item).into_iter().any(|c| has_control_key(c, keys))
}

fn pointing_menu_index(arr: &[Value]) -> Option<usize> {
    arr.iter().position(|m| label(m).to_lowercase().contains("pointing"))
}

fn first_channel(items: &[Value]) -> u8 {
    for item in items {
        if let Some(r) = value_ref(item) {
            return r.channel;
        }
        if let Some(arr) = item.get("content").and_then(|c| c.as_array()) {
            if arr.first().is_some_and(|v| v.is_object()) {
                return first_channel(arr);
            }
        }
    }
    0
}

fn insert_before_scroll_or_advanced(content: &mut Vec<Value>, section: Value) {
    let at = content
        .iter()
        .position(|i| matches!(label(i), "Scroll Settings" | "Advanced"))
        .unwrap_or(content.len());
    content.insert(at, section);
}

/// Append the Touchpad section to the Pointing Device menu when the firmware
/// advertises touchpad gestures (`FLAG3_PAD_GESTURES`) without listing them in
/// its definition `menus` yet (matching Keybard's `withTouchpadTaps`).
pub fn with_touchpad_taps(mut menus: Value, feature_flags3: u8, feature_flags4: u8) -> Value {
    if feature_flags3 & FLAG3_PAD_GESTURES == 0 {
        return menus;
    }
    let Some(arr) = menus.as_array_mut() else { return menus };
    let Some(idx) = pointing_menu_index(arr) else { return menus };
    if has_control_key(
        &arr[idx],
        &["id_pad_tap", "id_pad_two_finger", "id_pad_hold", "id_pad_hold_ms", "id_pad_touchpad_mode"],
    ) {
        return menus;
    }
    let Some(content) = arr[idx].get_mut("content").and_then(|c| c.as_array_mut()) else {
        return menus;
    };
    let ch = first_channel(content);
    let mut items = Vec::new();
    if feature_flags4 & FLAG4_PAD_MODE != 0 {
        items.push(json!({
            "label": "Touchpad Mode",
            "type": "dropdown",
            "options": ["Automatic", "Built-in gestures"],
            "content": ["id_pad_touchpad_mode", ch, 59],
        }));
    }
    items.push(json!({ "label": "Tap to Click", "type": "toggle", "showIf": "{app_pads_os} == 0", "content": ["id_pad_tap", ch, 51] }));
    items.push(json!({ "label": "Two-Finger Tap to Right Click", "type": "toggle", "showIf": "{app_pads_os} == 0", "content": ["id_pad_two_finger", ch, 52] }));
    items.push(json!({ "label": "Press and Hold to Drag", "type": "toggle", "showIf": "{app_pads_os} == 0", "content": ["id_pad_hold", ch, 53] }));
    items.push(json!({
        "label": "Hold Time (ms)",
        "type": "range",
        "options": [20, 1500, 10],
        "showIf": "{app_pads_os} == 0",
        "disabledIf": "{id_pad_hold} == 0",
        "content": ["id_pad_hold_ms", ch, 54],
    }));
    if feature_flags3 & FLAG3_PAD_TUNING != 0 {
        items.push(json!({ "label": "Dead Zone (0.01 mm)", "type": "range", "options": [20, 400, 10], "content": ["id_pad_dead_zone", ch, 55] }));
        items.push(json!({ "label": "Tap Time (ms)", "type": "range", "options": [50, 1000, 25], "content": ["id_pad_tap_ms", ch, 56] }));
        items.push(json!({ "label": "Scroll Start (0.01 mm)", "type": "range", "options": [0, 1000, 5], "content": ["id_pad_scroll_start", ch, 57] }));
        if feature_flags4 & FLAG4_PAD_PALM != 0 {
            items.push(json!({ "label": "Palm Rejection (channels)", "type": "range", "options": [0, 56, 1], "content": ["id_pad_palm", ch, 58] }));
        }
    }
    let section = json!({
        "label": "Touchpad",
        "note": "Windows and Linux use their native Precision Touchpad Protocol by default. Mac is limited to the built-in gestures from Azoteq.",
        "content": items,
    });
    insert_before_scroll_or_advanced(content, section);
    menus
}

/// Append per-hand Scroll speed controls (`id_left_scroll_speed`,
/// `id_right_scroll_speed`, VIA 61 & 62) when `FLAG4_SCROLL_SPEED` is set and
/// the menu doesn't list them already (matching Keybard's `withScrollSpeed`).
pub fn with_scroll_speed(mut menus: Value, feature_flags4: u8) -> Value {
    if feature_flags4 & FLAG4_SCROLL_SPEED == 0 {
        return menus;
    }
    let Some(arr) = menus.as_array_mut() else { return menus };
    let Some(idx) = pointing_menu_index(arr) else { return menus };
    if has_control_key(&arr[idx], &["id_left_scroll_speed"]) {
        return menus;
    }
    let Some(content) = arr[idx].get_mut("content").and_then(|c| c.as_array_mut()) else {
        return menus;
    };
    let ch = first_channel(content);
    let section = json!({
        "label": "Scroll speed",
        "content": [
            { "label": "Left Scroll speed", "type": "range", "options": [20, 1000, 1], "content": ["id_left_scroll_speed", ch, 61] },
            { "label": "Right Scroll speed", "type": "range", "options": [20, 1000, 1], "content": ["id_right_scroll_speed", ch, 62] },
        ],
    });
    insert_before_scroll_or_advanced(content, section);
    menus
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn menus() -> Value {
        json!([{
            "label": "Pointing",
            "content": [{
                "label": "Left",
                "content": [
                    {"label": "DPI", "type": "range", "options": [100, 3200], "content": ["id_left_dpi", 0, 1]},
                    {"label": "Scroll", "type": "toggle", "content": ["id_left_scroll", 0, 2]},
                    {"label": "Mode", "type": "dropdown", "options": ["A", "B"], "content": ["id_mode", 0, 3],
                     "showIf": "{id_left_scroll} == 1"},
                    {"label": "Color", "type": "color", "content": ["id_layer0_color", 0, 32]}
                ]
            }]
        }, "qmk_rgblight"])
    }

    #[test]
    fn walk_controls() {
        let m = menus();
        let c = controls(&m);
        assert_eq!(c.len(), 4);
        assert_eq!(c[0].1, ValueRef { key: "id_left_dpi".into(), channel: 0, value_id: 1 });
        assert_eq!(byte_width(c[0].0), 2);
        assert_eq!(byte_width(c[1].0), 1);
        assert_eq!(byte_width(c[3].0), 2);
        assert_eq!(dropdown_options(c[2].0), vec![("A".into(), 0), ("B".into(), 1)]);
    }

    #[test]
    fn show_if() {
        let mut v = HashMap::new();
        assert_eq!(evaluate_condition("{x} == 1", &v), None);
        v.insert("x".to_string(), 1);
        assert_eq!(evaluate_condition("{x} == 1", &v), Some(true));
        assert_eq!(evaluate_condition("{x} > 1", &v), Some(false));
        assert_eq!(evaluate_condition("garbage", &v), Some(true));
    }

    #[test]
    fn ints() {
        assert_eq!(bytes_to_int(&[0x34, 0x12]), 0x1234);
        assert_eq!(int_to_bytes(0x1234, 2), vec![0x34, 0x12]);
    }

    #[test]
    fn augment_pointing_menus_and_helpers() {
        let m = json!([{
            "label": "Pointing Device",
            "content": [
                {"label": "Left Pointer", "content": [{"label": "DPI", "type": "dropdown", "options": ["200"], "content": ["id_left_dpi", 2, 0]}]},
                {"label": "Scroll Settings", "content": []}
            ]
        }]);
        let m = with_touchpad_taps(m, FLAG3_PAD_GESTURES | FLAG3_PAD_TUNING, FLAG4_PAD_MODE | FLAG4_PAD_PALM);
        let m = with_scroll_speed(m, FLAG4_SCROLL_SPEED);
        let ctrls = controls(&m);
        assert!(ctrls.iter().any(|(_, r)| r.key == "id_pad_touchpad_mode" && r.channel == 2 && r.value_id == 59));
        assert!(ctrls.iter().any(|(_, r)| r.key == "id_pad_palm" && r.channel == 2 && r.value_id == 58));
        assert!(ctrls.iter().any(|(_, r)| r.key == "id_left_scroll_speed" && r.channel == 2 && r.value_id == 61));
        assert!(ctrls.iter().any(|(_, r)| r.key == "id_right_scroll_speed" && r.channel == 2 && r.value_id == 62));

        // Idempotent when called again.
        let m2 = with_touchpad_taps(m.clone(), FLAG3_PAD_GESTURES | FLAG3_PAD_TUNING, FLAG4_PAD_MODE | FLAG4_PAD_PALM);
        let m2 = with_scroll_speed(m2, FLAG4_SCROLL_SPEED);
        assert_eq!(m, m2);

        let hold_ms = ctrls.iter().find(|(_, r)| r.key == "id_pad_hold_ms").unwrap().0;
        assert_eq!(range_step(hold_ms), Some(10));
        let mut vals = HashMap::new();
        vals.insert("id_pad_hold".into(), 0);
        assert!(is_disabled(hold_ms, &vals));
        vals.insert("id_pad_hold".into(), 1);
        assert!(!is_disabled(hold_ms, &vals));
    }
}
