//! `.svil` layout files: Svalboard's native save format (viable-gui's layout
//! JSON), as written by Keybard's `kbinfoToSvil` and read by `svilToKBINFO`.
//!
//! Keycodes are written by name (see [`KeycodeNames`]); a keycode without a
//! name is written as `0x1234`, meaningful only in the file's `keycode_version`.

use super::keycode_names::KeycodeNames;
use super::macros::MacroAction;
use super::menus::CustomValueEntry;
use super::{AltRepeatKey, Combo, KeyOverride, Leader, OneShot, TapDance, COMBO_ENABLED, KEY_OVERRIDE_ENABLED};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Everything a `.svil` file holds, with numeric keycodes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SvilData {
    pub uid: u64,
    pub svil_protocol: u32,
    pub via_protocol: u16,
    pub keycode_version: String,
    /// `[layer][row][col]`.
    pub layout: Vec<Vec<Vec<u16>>>,
    pub macros: Vec<Vec<MacroAction>>,
    pub tap_dances: Vec<TapDance>,
    pub combos: Vec<Combo>,
    pub key_overrides: Vec<KeyOverride>,
    pub alt_repeat_keys: Vec<AltRepeatKey>,
    pub leaders: Vec<Leader>,
    pub one_shot: Option<OneShot>,
    /// QMK settings by QSID.
    pub settings: BTreeMap<u16, u32>,
    /// Named layers only.
    pub layer_names: BTreeMap<usize, String>,
    pub custom_values: Vec<CustomValueEntry>,
    pub menus: Option<Value>,
    pub fragments: Option<Value>,
    pub composition: Option<Value>,
}

/// Build the `[layer][row][col]` layout from flat per-layer keymaps.
pub fn layout_from_flat(layers: &[Vec<u16>], rows: usize, cols: usize) -> Vec<Vec<Vec<u16>>> {
    layers
        .iter()
        .map(|l| (0..rows).map(|r| (0..cols).map(|c| l.get(r * cols + c).copied().unwrap_or(0)).collect()).collect())
        .collect()
}

/// Flatten a `[layer][row][col]` layout into per-layer keymaps.
pub fn flat_from_layout(layout: &[Vec<Vec<u16>>]) -> Vec<Vec<u16>> {
    layout.iter().map(|l| l.iter().flatten().copied().collect()).collect()
}

const UID_PLACEHOLDER: &str = "UID_PLACEHOLDER_FOR_BIGINT";

impl SvilData {
    /// Serialize as pretty-printed `.svil` JSON (2-space indent, like Keybard).
    pub fn to_json(&self, names: &KeycodeNames) -> String {
        let s = |k: u16| Value::String(names.stringify(k));
        let mut root = Map::new();
        root.insert("version".into(), json!(1));
        // The UID is written as a bare number; Keybard reads it with full 64-bit precision.
        root.insert("uid".into(), json!(UID_PLACEHOLDER));
        root.insert(
            "layout".into(),
            Value::Array(
                self.layout
                    .iter()
                    .map(|l| Value::Array(l.iter().map(|r| Value::Array(r.iter().map(|&k| s(k)).collect())).collect()))
                    .collect(),
            ),
        );
        root.insert("encoder_layout".into(), json!([]));
        root.insert("layout_options".into(), json!(-1));
        root.insert(
            "macro".into(),
            Value::Array(
                self.macros.iter().map(|m| Value::Array(m.iter().map(|a| a.to_json(names)).collect())).collect(),
            ),
        );
        root.insert("svil_protocol".into(), json!(self.svil_protocol.max(1)));
        root.insert("keycode_version".into(), json!(self.keycode_version));
        root.insert("via_protocol".into(), json!(if self.via_protocol == 0 { 12 } else { self.via_protocol }));
        root.insert(
            "tap_dance".into(),
            Value::Array(
                self.tap_dances
                    .iter()
                    .map(|t| {
                        json!({
                            "on": t.enabled,
                            "on_tap": s(t.on_tap),
                            "on_hold": s(t.on_hold),
                            "on_double_tap": s(t.on_double_tap),
                            "on_tap_hold": s(t.on_tap_hold),
                            "tapping_term": t.tapping_term,
                        })
                    })
                    .collect(),
            ),
        );
        root.insert(
            "combo".into(),
            Value::Array(
                self.combos
                    .iter()
                    .map(|c| {
                        json!({
                            "on": c.options & COMBO_ENABLED != 0,
                            "keys": c.keys.iter().map(|&k| s(k)).collect::<Vec<_>>(),
                            "output": s(c.output),
                            "combo_term": c.options & 0x7FFF,
                        })
                    })
                    .collect(),
            ),
        );
        root.insert(
            "key_override".into(),
            Value::Array(
                self.key_overrides
                    .iter()
                    .map(|k| {
                        json!({
                            "on": k.options & KEY_OVERRIDE_ENABLED != 0,
                            "trigger": s(k.trigger),
                            "replacement": s(k.replacement),
                            "layers": k.layers,
                            "trigger_mods": k.trigger_mods,
                            "negative_mod_mask": k.negative_mod_mask,
                            "suppressed_mods": k.suppressed_mods,
                            "options": k.options,
                        })
                    })
                    .collect(),
            ),
        );
        root.insert(
            "alt_repeat_key".into(),
            Value::Array(
                self.alt_repeat_keys
                    .iter()
                    .map(|a| {
                        json!({
                            "on": true,
                            "keycode": s(a.keycode),
                            "alt_keycode": s(a.alt_keycode),
                            "allowed_mods": a.allowed_mods,
                            "options": a.options,
                        })
                    })
                    .collect(),
            ),
        );
        root.insert(
            "leader".into(),
            Value::Array(
                self.leaders
                    .iter()
                    .map(|l| {
                        json!({
                            "on": true,
                            "sequence": l.sequence.iter().filter(|&&k| k != 0).map(|&k| s(k)).collect::<Vec<_>>(),
                            "output": s(l.output),
                            "options": l.options,
                        })
                    })
                    .collect(),
            ),
        );
        root.insert(
            "settings".into(),
            Value::Object(self.settings.iter().map(|(q, v)| (q.to_string(), json!(v))).collect()),
        );
        if let Some(os) = &self.one_shot {
            root.insert("oneshot".into(), json!({"timeout": os.timeout, "tap_toggle": os.tap_toggle}));
        }
        if let Some(f) = &self.fragments {
            root.insert("fragments".into(), f.clone());
        }
        if let Some(c) = &self.composition {
            root.insert("composition".into(), c.clone());
        }
        if let Some(m) = &self.menus {
            root.insert("menus".into(), m.clone());
        }
        if !self.layer_names.is_empty() {
            let layer: Map<String, Value> =
                self.layer_names.iter().map(|(i, n)| (i.to_string(), Value::String(n.clone()))).collect();
            root.insert("cosmetic".into(), json!({ "layer": layer }));
        }
        if !self.custom_values.is_empty() {
            root.insert(
                "custom_values".into(),
                Value::Array(
                    self.custom_values
                        .iter()
                        .map(|c| json!({"key": c.key, "channel": c.channel, "valueId": c.value_id, "data": c.data}))
                        .collect(),
                ),
            );
        }
        let text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
        text.replacen(&format!("\"{UID_PLACEHOLDER}\""), &self.uid.to_string(), 1)
    }

    /// Parse a `.svil` (or legacy `.viable`) file.
    pub fn from_json(text: &str, names: &KeycodeNames) -> Result<SvilData, String> {
        let js: Value = serde_json::from_str(text).map_err(|e| format!("Invalid JSON: {e}"))?;
        let obj = js.as_object().ok_or("Unknown file format: expected a JSON object.")?;
        if !obj.contains_key("uid") {
            return Err("Unknown file format. Expected a .svil file.".into());
        }
        let is_svil = ["sval_protocol", "svil_protocol", "viable_protocol"].iter().any(|k| obj.contains_key(*k))
            || (obj.get("version").and_then(|v| v.as_u64()) == Some(1) && !obj.contains_key("vial_protocol"));
        if !is_svil {
            return Err("This is a Vial .vil file, not a .svil file.".into());
        }
        let key = |v: &Value| -> Result<u16, String> {
            match v {
                Value::String(s) => names.parse(s).ok_or_else(|| format!("Unrecognized keycode \"{s}\"")),
                Value::Number(n) => match n.as_i64() {
                    Some(-1) => Ok(0),
                    Some(k) if (0..=0xFFFF).contains(&k) => Ok(k as u16),
                    _ => Err(format!("Invalid keycode {n}")),
                },
                Value::Null => Ok(0),
                _ => Err("Invalid keycode".into()),
            }
        };
        let num = |v: Option<&Value>| v.and_then(|x| x.as_u64()).unwrap_or(0);
        let list = |name: &str| -> Result<Vec<Value>, String> {
            match obj.get(name) {
                None | Some(Value::Null) => Ok(Vec::new()),
                Some(Value::Array(a)) => Ok(a.clone()),
                Some(_) => Err(format!("Invalid layout file: {name} must be a list.")),
            }
        };

        let mut data = SvilData {
            uid: match obj.get("uid") {
                Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
                Some(Value::String(s)) => s.parse().unwrap_or(0),
                _ => 0,
            },
            svil_protocol: ["sval_protocol", "svil_protocol", "viable_protocol"]
                .iter()
                .find_map(|k| obj.get(*k).and_then(|v| v.as_u64()))
                .unwrap_or(1) as u32,
            via_protocol: obj.get("via_protocol").and_then(|v| v.as_u64()).unwrap_or(12) as u16,
            keycode_version: obj
                .get("keycode_version")
                .and_then(|v| v.as_str())
                .unwrap_or(super::KEYCODE_VERSION)
                .to_string(),
            ..Default::default()
        };

        let layout = list("layout")?;
        if layout.is_empty() {
            return Err("Invalid layout file: expected a non-empty layout.".into());
        }
        let mut dims: Option<(usize, usize)> = None;
        for layer in &layout {
            let rows = layer.as_array().ok_or("Invalid layout file: a layer must be a list of rows.")?;
            let mut out_layer = Vec::new();
            for row in rows {
                let cols = row.as_array().ok_or("Invalid layout file: a row must be a list of keys.")?;
                out_layer.push(cols.iter().map(&key).collect::<Result<Vec<_>, _>>()?);
            }
            let d = (out_layer.len(), out_layer.first().map(|r| r.len()).unwrap_or(0));
            if d.0 == 0 || d.1 == 0 || out_layer.iter().any(|r| r.len() != d.1) || dims.is_some_and(|p| p != d) {
                return Err("Invalid layout file: all layers must have the same rectangular matrix.".into());
            }
            dims = Some(d);
            data.layout.push(out_layer);
        }

        for m in list("macro")? {
            let mut actions = Vec::new();
            for group in m.as_array().ok_or("Invalid layout file: a macro must be a list.")? {
                actions.extend(MacroAction::from_json(group, names)?);
            }
            data.macros.push(actions);
        }
        for t in list("tap_dance")? {
            data.tap_dances.push(TapDance {
                enabled: t.get("on").and_then(|v| v.as_bool()).unwrap_or(true),
                on_tap: t.get("on_tap").map(&key).transpose()?.unwrap_or(0),
                on_hold: t.get("on_hold").map(&key).transpose()?.unwrap_or(0),
                on_double_tap: t.get("on_double_tap").map(&key).transpose()?.unwrap_or(0),
                on_tap_hold: t.get("on_tap_hold").map(&key).transpose()?.unwrap_or(0),
                tapping_term: (num(t.get("tapping_term")) & 0x7FFF) as u16,
            });
        }
        for c in list("combo")? {
            let mut keys = [0u16; 4];
            if let Some(arr) = c.get("keys").and_then(|k| k.as_array()) {
                for (i, k) in arr.iter().take(4).enumerate() {
                    keys[i] = key(k)?;
                }
            }
            let on = c.get("on").and_then(|v| v.as_bool()).unwrap_or(true);
            data.combos.push(Combo {
                keys,
                output: c.get("output").map(&key).transpose()?.unwrap_or(0),
                options: (num(c.get("combo_term")) as u16 & 0x7FFF) | if on { COMBO_ENABLED } else { 0 },
            });
        }
        for k in list("key_override")? {
            let mut options = num(k.get("options")) as u8;
            match k.get("on").and_then(|v| v.as_bool()) {
                Some(true) => options |= KEY_OVERRIDE_ENABLED,
                Some(false) => options &= !KEY_OVERRIDE_ENABLED,
                None => {}
            }
            data.key_overrides.push(KeyOverride {
                trigger: k.get("trigger").map(&key).transpose()?.unwrap_or(0),
                replacement: k.get("replacement").map(&key).transpose()?.unwrap_or(0),
                layers: k.get("layers").and_then(|v| v.as_u64()).unwrap_or(0xFFFF) as u32,
                trigger_mods: num(k.get("trigger_mods")) as u8,
                negative_mod_mask: num(k.get("negative_mod_mask")) as u8,
                suppressed_mods: num(k.get("suppressed_mods")) as u8,
                options,
            });
        }
        for a in list("alt_repeat_key")? {
            data.alt_repeat_keys.push(AltRepeatKey {
                keycode: a.get("keycode").map(&key).transpose()?.unwrap_or(0),
                alt_keycode: a.get("alt_keycode").map(&key).transpose()?.unwrap_or(0),
                allowed_mods: num(a.get("allowed_mods")) as u8,
                options: num(a.get("options")) as u8,
            });
        }
        for l in list("leader")? {
            let mut sequence = [0u16; 5];
            if let Some(arr) = l.get("sequence").and_then(|k| k.as_array()) {
                for (i, k) in arr.iter().take(5).enumerate() {
                    sequence[i] = key(k)?;
                }
            }
            data.leaders.push(Leader {
                sequence,
                output: l.get("output").map(&key).transpose()?.unwrap_or(0),
                options: num(l.get("options")) as u16,
            });
        }
        if let Some(os) = obj.get("oneshot").filter(|v| v.is_object()) {
            data.one_shot =
                Some(OneShot { timeout: num(os.get("timeout")) as u16, tap_toggle: num(os.get("tap_toggle")) as u8 });
        }
        if let Some(settings) = obj.get("settings").and_then(|s| s.as_object()) {
            for (q, v) in settings {
                if let (Ok(q), Some(v)) = (q.parse::<u16>(), v.as_u64()) {
                    data.settings.insert(q, v as u32);
                }
            }
        }
        if let Some(layers) = obj.get("cosmetic").and_then(|c| c.get("layer")).and_then(|l| l.as_object()) {
            for (i, n) in layers {
                if let (Ok(i), Some(n)) = (i.parse::<usize>(), n.as_str()) {
                    if !n.is_empty() {
                        data.layer_names.insert(i, n.to_string());
                    }
                }
            }
        }
        for cv in list("custom_values")? {
            let (Some(k), Some(ch), Some(id)) = (
                cv.get("key").and_then(|v| v.as_str()),
                cv.get("channel").and_then(|v| v.as_u64()),
                cv.get("valueId").and_then(|v| v.as_u64()),
            ) else {
                continue;
            };
            let bytes = cv
                .get("data")
                .and_then(|d| d.as_array())
                .map(|a| a.iter().filter_map(|b| b.as_u64()).map(|b| b as u8).collect())
                .unwrap_or_default();
            data.custom_values.push(CustomValueEntry { key: k.to_string(), channel: ch as u8, value_id: id as u8, data: bytes });
        }
        data.menus = obj.get("menus").cloned();
        data.fragments = obj.get("fragments").cloned();
        data.composition = obj.get("composition").cloned();
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svil::keycode_names::CustomKeycode;

    fn sample() -> SvilData {
        SvilData {
            uid: 0xF00D_CAFE_DEAD_BEEF,
            svil_protocol: 3,
            via_protocol: 12,
            keycode_version: "0.0.9".into(),
            layout: vec![vec![vec![0x04, 0x05], vec![0x5221, 0x7E40]], vec![vec![0x01, 0x00], vec![0x0204, 0x412C]]],
            macros: vec![vec![MacroAction::Text("hi".into()), MacroAction::Tap(0x28)], vec![]],
            tap_dances: vec![TapDance { on_tap: 4, on_hold: 5, tapping_term: 200, enabled: true, ..Default::default() }],
            combos: vec![Combo { keys: [4, 5, 0, 0], output: 6, options: 0x8032 }],
            key_overrides: vec![KeyOverride { trigger: 4, replacement: 5, layers: 0xFFFF, options: 0x87, ..Default::default() }],
            alt_repeat_keys: vec![AltRepeatKey { keycode: 4, alt_keycode: 5, allowed_mods: 1, options: 2 }],
            leaders: vec![Leader { sequence: [4, 5, 0, 0, 0], output: 6, options: 1 }],
            one_shot: Some(OneShot { timeout: 3000, tap_toggle: 5 }),
            settings: [(1u16, 1u32), (7, 200)].into_iter().collect(),
            layer_names: [(0usize, "Base".to_string()), (3, "Nav".to_string())].into_iter().collect(),
            custom_values: vec![CustomValueEntry { key: "id_left_dpi".into(), channel: 0, value_id: 1, data: vec![0x20, 0x03] }],
            menus: None,
            fragments: None,
            composition: None,
        }
    }

    fn names() -> KeycodeNames {
        KeycodeNames::new(vec![CustomKeycode { name: "SV_LEFT_DPI_INC".into(), ..Default::default() }])
    }

    #[test]
    fn roundtrip() {
        let d = sample();
        let text = d.to_json(&names());
        assert!(text.contains("\"uid\": 17297704939806375663"));
        assert!(text.contains("\"SV_LEFT_DPI_INC\""));
        assert!(text.contains("\"MO(1)\""));
        let back = SvilData::from_json(&text, &names()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn rejects_bad_files() {
        assert!(SvilData::from_json("{}", &names()).is_err());
        assert!(SvilData::from_json(r#"{"uid": 1, "vial_protocol": 6, "layout": [[["KC_A"]]]}"#, &names()).is_err());
        assert!(SvilData::from_json(r#"{"uid": 1, "svil_protocol": 3, "layout": [[["NOPE"]]]}"#, &names()).is_err());
        assert!(SvilData::from_json(r#"{"uid": 1, "svil_protocol": 3, "layout": [[["KC_A"]], [["KC_A", "KC_B"]]]}"#, &names()).is_err());
    }

    #[test]
    fn layout_helpers() {
        let flat = vec![vec![1, 2, 3, 4, 5, 6]];
        let l = layout_from_flat(&flat, 2, 3);
        assert_eq!(l, vec![vec![vec![1, 2, 3], vec![4, 5, 6]]]);
        assert_eq!(flat_from_layout(&l), flat);
    }
}
