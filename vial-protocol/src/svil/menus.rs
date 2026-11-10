//! VIA3 custom UI menus (the `menus` of a keyboard definition), which SVIL
//! boards use for their own settings (pointing devices, layer colors, ...).
//!
//! A menu item is either a section `{label, content: [items...], showIf?}` or a
//! control `{label, type, options?, content: ["id_key", channel, value_id], showIf?}`.
//! Values are read and written with the VIA custom value commands.

use serde_json::Value;
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

/// `[min, max]` of a range control (default 0..255).
pub fn range(item: &Value) -> (i64, i64) {
    let opts = item.get("options").and_then(|o| o.as_array());
    let get = |i: usize, d: i64| opts.and_then(|o| o.get(i)).and_then(|v| v.as_i64()).unwrap_or(d);
    (get(0, 0), get(1, 255))
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
}
