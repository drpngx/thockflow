//! Vial `.vil` layout files (as written by vial-gui), and their migration to
//! SVIL keyboards.
//!
//! A `.vil` stores keycodes by Vial/QMK name, so a layout saved from a board's
//! Vial firmware carries across to its Svalboard QMK (SVIL) firmware. Vial names
//! a board's own keycodes by position (`USER00`, `USER01`, ...); on Svalboard's
//! Vial firmware those positions are known ([`SVALBOARD_VIAL_CUSTOM_KEYCODES`])
//! and become the SVIL board's named keycodes (`SV_SNIPER_2`, ...).
//!
//! Format reference: vial-gui `layout_editor` save/restore, and Keybard's
//! `kbinfoToVIL` / `vilToKBINFO`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::svil::file::SvilData;
use crate::svil::keycode_names::KeycodeNames;
use crate::svil::macros::MacroAction;
use crate::svil::{Combo, KeyOverride, TapDance, COMBO_ENABLED};
use crate::{ComboEntry, TapDanceEntry};

/// `VIAL_KEYBOARD_UID` of Svalboard's Vial firmware; Svalboard QMK reports the same UID.
pub const SVALBOARD_VIAL_UID: u64 = 5199957870438586395;

/// The keycodes at Svalboard Vial's `USER00..` positions (svalboard/vial-qmk, keymap "vial").
pub const SVALBOARD_VIAL_CUSTOM_KEYCODES: &[&str] = &[
    "SV_LEFT_DPI_INC",
    "SV_LEFT_DPI_DEC",
    "SV_RIGHT_DPI_INC",
    "SV_RIGHT_DPI_DEC",
    "SV_LEFT_SCROLL_TOGGLE",
    "SV_RIGHT_SCROLL_TOGGLE",
    "SV_RECALIBRATE_POINTER",
    "SV_MH_CHANGE_TIMEOUTS",
    "SV_CAPS_WORD",
    "SV_AXIS_SCROLL_LOCK",
    "SV_TOGGLE_23_67",
    "SV_TOGGLE_45_67",
    "SV_SNIPER_2",
    "SV_SNIPER_3",
    "SV_SNIPER_5",
    "SV_SCROLL_HOLD",
    "SV_SCROLL_TOGGLE",
    "SV_OUTPUT_STATUS",
    "SV_TOGGLE_AUTOMOUSE",
    "SV_TURBO_SCAN",
];

/// Vial's legacy `USER00` (QK_KB) code; `USERnn` is `VIAL_USER + nn`.
pub const VIAL_USER: u16 = 0x7E00;
pub const VIAL_USER_COUNT: u16 = 64;

/// A Vial key override (`layers` is 16 bits in Vial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VialKeyOverride {
    pub trigger: u16,
    pub replacement: u16,
    pub layers: u16,
    pub trigger_mods: u8,
    pub negative_mod_mask: u8,
    pub suppressed_mods: u8,
    /// Bit 7: enabled.
    pub options: u8,
}

impl VialKeyOverride {
    /// Vial's dynamic entry form `[trigger:2][replacement:2][layers:2][trigger_mods][negative][suppressed][options]`.
    pub fn from_bytes(d: &[u8]) -> Option<Self> {
        if d.len() < 10 {
            return None;
        }
        Some(Self {
            trigger: u16::from_le_bytes([d[0], d[1]]),
            replacement: u16::from_le_bytes([d[2], d[3]]),
            layers: u16::from_le_bytes([d[4], d[5]]),
            trigger_mods: d[6],
            negative_mod_mask: d[7],
            suppressed_mods: d[8],
            options: d[9],
        })
    }
}

/// Number of tap dances, combos and key overrides from Vial's
/// `DynamicEntryOp::GetNumberOfEntries` reply `[td][combo][ko]...`.
pub fn parse_dynamic_counts(resp: &[u8]) -> (u8, u8, u8) {
    let g = |i: usize| resp.get(i).copied().unwrap_or(0);
    (g(0), g(1), g(2))
}

/// Entry bytes of a Vial dynamic entry GET reply `[status][entry...]`, `None` if refused.
pub fn parse_dynamic_entry(resp: &[u8]) -> Option<&[u8]> {
    (resp.first() == Some(&0)).then(|| &resp[1..])
}

/// Everything a `.vil` file holds, with numeric keycodes (Vial numbering).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VilData {
    pub uid: u64,
    pub vial_protocol: u32,
    pub via_protocol: u16,
    /// `[layer][row][col]`; `None` marks a position outside the matrix (`-1`).
    pub layout: Vec<Vec<Vec<Option<u16>>>>,
    /// `[layer][encoder] = (ccw, cw)`.
    pub encoder_layout: Vec<Vec<(u16, u16)>>,
    pub layout_options: i64,
    pub macros: Vec<Vec<MacroAction>>,
    pub tap_dances: Vec<TapDanceEntry>,
    pub combos: Vec<ComboEntry>,
    pub key_overrides: Vec<VialKeyOverride>,
    /// QMK settings by QSID.
    pub settings: BTreeMap<u16, u32>,
    /// Layer names (not part of vial-gui's format; written under `cosmetic`, which it ignores).
    pub layer_names: BTreeMap<usize, String>,
}

const UID_PLACEHOLDER: &str = "UID_PLACEHOLDER_FOR_BIGINT";

fn names() -> KeycodeNames {
    KeycodeNames::default()
}

impl VilData {
    pub fn to_json(&self) -> String {
        let n = names();
        let s = |k: u16| Value::String(n.stringify(k));
        let mut root = Map::new();
        root.insert("version".into(), json!(1));
        root.insert("uid".into(), json!(UID_PLACEHOLDER));
        root.insert(
            "layout".into(),
            Value::Array(
                self.layout
                    .iter()
                    .map(|l| {
                        Value::Array(
                            l.iter()
                                .map(|r| Value::Array(r.iter().map(|k| k.map(s).unwrap_or(json!(-1))).collect()))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        );
        root.insert(
            "encoder_layout".into(),
            Value::Array(
                self.encoder_layout
                    .iter()
                    .map(|l| Value::Array(l.iter().map(|(a, b)| json!([n.stringify(*a), n.stringify(*b)])).collect()))
                    .collect(),
            ),
        );
        root.insert("layout_options".into(), json!(self.layout_options));
        root.insert(
            "macro".into(),
            Value::Array(self.macros.iter().map(|m| Value::Array(m.iter().map(|a| a.to_json(&n)).collect())).collect()),
        );
        root.insert("vial_protocol".into(), json!(self.vial_protocol));
        root.insert("via_protocol".into(), json!(self.via_protocol));
        root.insert(
            "tap_dance".into(),
            Value::Array(
                self.tap_dances
                    .iter()
                    .map(|t| json!([s(t.on_tap), s(t.on_hold), s(t.on_double_tap), s(t.on_tap_hold), t.tapping_term]))
                    .collect(),
            ),
        );
        root.insert(
            "combo".into(),
            Value::Array(
                self.combos
                    .iter()
                    .map(|c| Value::Array(c.keys.iter().chain(std::iter::once(&c.output)).map(|&k| s(k)).collect()))
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
            "settings".into(),
            Value::Object(self.settings.iter().map(|(q, v)| (q.to_string(), json!(v))).collect()),
        );
        if !self.layer_names.is_empty() {
            let layer: Map<String, Value> =
                self.layer_names.iter().map(|(i, n)| (i.to_string(), Value::String(n.clone()))).collect();
            root.insert("cosmetic".into(), json!({ "layer": layer }));
        }
        let text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
        text.replacen(&format!("\"{UID_PLACEHOLDER}\""), &self.uid.to_string(), 1)
    }

    /// Parse a `.vil` file.
    pub fn from_json(text: &str) -> Result<VilData, String> {
        let js: Value = serde_json::from_str(text).map_err(|e| format!("Invalid JSON: {e}"))?;
        let obj = js.as_object().ok_or("Unknown file format: expected a JSON object.")?;
        if !obj.contains_key("uid") || !obj.contains_key("layout") {
            return Err("Unknown file format. Expected a Vial .vil file.".into());
        }
        let n = names();
        let key = |v: &Value| -> Result<u16, String> {
            match v {
                Value::String(s) => n.parse(s).ok_or_else(|| format!("Unrecognized keycode \"{s}\"")),
                Value::Number(x) => x
                    .as_u64()
                    .filter(|k| *k <= 0xFFFF)
                    .map(|k| k as u16)
                    .ok_or_else(|| format!("Invalid keycode {x}")),
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
        let mut data = VilData {
            uid: match obj.get("uid") {
                Some(Value::Number(x)) => x.as_u64().unwrap_or(0),
                Some(Value::String(s)) => s.parse().unwrap_or(0),
                _ => 0,
            },
            vial_protocol: num(obj.get("vial_protocol")) as u32,
            via_protocol: num(obj.get("via_protocol")) as u16,
            layout_options: obj.get("layout_options").and_then(|v| v.as_i64()).unwrap_or(-1),
            ..Default::default()
        };
        let mut dims: Option<(usize, usize)> = None;
        for layer in list("layout")? {
            let rows = layer.as_array().ok_or("Invalid layout file: a layer must be a list of rows.")?;
            let mut out = Vec::new();
            for row in rows {
                let cols = row.as_array().ok_or("Invalid layout file: a row must be a list of keys.")?;
                out.push(
                    cols.iter()
                        .map(|k| if k.as_i64() == Some(-1) { Ok(None) } else { key(k).map(Some) })
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            let d = (out.len(), out.first().map(|r| r.len()).unwrap_or(0));
            if d.0 == 0 || d.1 == 0 || out.iter().any(|r| r.len() != d.1) || dims.is_some_and(|p| p != d) {
                return Err("Invalid layout file: all layers must have the same rectangular matrix.".into());
            }
            dims = Some(d);
            data.layout.push(out);
        }
        if data.layout.is_empty() {
            return Err("Invalid layout file: expected a non-empty layout.".into());
        }
        for l in list("encoder_layout")? {
            let mut enc = Vec::new();
            for e in l.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                let pair = e.as_array().ok_or("Invalid encoder layout")?;
                let g = |i: usize| pair.get(i).map(&key).transpose().map(|k| k.unwrap_or(0));
                enc.push((g(0)?, g(1)?));
            }
            data.encoder_layout.push(enc);
        }
        for m in list("macro")? {
            let mut actions = Vec::new();
            for group in m.as_array().ok_or("Invalid layout file: a macro must be a list.")? {
                actions.extend(MacroAction::from_json(group, &n)?);
            }
            data.macros.push(actions);
        }
        for t in list("tap_dance")? {
            let a = t.as_array().ok_or("Invalid tap dance")?;
            let k = |i: usize| a.get(i).map(&key).transpose().map(|k| k.unwrap_or(0));
            data.tap_dances.push(TapDanceEntry {
                on_tap: k(0)?,
                on_hold: k(1)?,
                on_double_tap: k(2)?,
                on_tap_hold: k(3)?,
                tapping_term: num(a.get(4)) as u16,
            });
        }
        for c in list("combo")? {
            let a = c.as_array().ok_or("Invalid combo")?;
            let k = |i: usize| a.get(i).map(&key).transpose().map(|k| k.unwrap_or(0));
            data.combos.push(ComboEntry { keys: [k(0)?, k(1)?, k(2)?, k(3)?], output: k(4)? });
        }
        for k in list("key_override")? {
            data.key_overrides.push(VialKeyOverride {
                trigger: k.get("trigger").map(&key).transpose()?.unwrap_or(0),
                replacement: k.get("replacement").map(&key).transpose()?.unwrap_or(0),
                layers: k.get("layers").and_then(|v| v.as_u64()).unwrap_or(0xFFFF) as u16,
                trigger_mods: num(k.get("trigger_mods")) as u8,
                negative_mod_mask: num(k.get("negative_mod_mask")) as u8,
                suppressed_mods: num(k.get("suppressed_mods")) as u8,
                options: num(k.get("options")) as u8,
            });
        }
        if let Some(settings) = obj.get("settings").and_then(|s| s.as_object()) {
            for (q, v) in settings {
                if let (Ok(q), Some(v)) = (q.parse::<u16>(), v.as_u64()) {
                    data.settings.insert(q, v as u32);
                }
            }
        }
        if let Some(layers) = obj.get("cosmetic").and_then(|c| c.get("layer")).and_then(|l| l.as_object()) {
            for (i, nm) in layers {
                if let (Ok(i), Some(nm)) = (i.parse::<usize>(), nm.as_str()) {
                    if !nm.is_empty() {
                        data.layer_names.insert(i, nm.to_string());
                    }
                }
            }
        }
        Ok(data)
    }

    /// Whether this layout was saved from Svalboard's Vial firmware.
    pub fn is_svalboard(&self) -> bool {
        self.uid == SVALBOARD_VIAL_UID
    }

    /// Convert for an SVIL keyboard whose own keycodes are `board`.
    ///
    /// `USERnn` keycodes become the board's named keycodes when the file comes from
    /// Svalboard's Vial firmware (positions Vial left unassigned become `KC_NO`);
    /// from any other board they are kept as they are and counted as foreign.
    pub fn to_svil(&self, board: &KeycodeNames) -> (SvilData, MigrationReport) {
        let mut report = MigrationReport { svalboard: self.is_svalboard(), ..Default::default() };
        let mut map = |k: u16| -> u16 {
            if !(VIAL_USER..VIAL_USER + VIAL_USER_COUNT).contains(&k) {
                return k;
            }
            if !report.svalboard {
                report.foreign_custom_keycodes += 1;
                return k;
            }
            let idx = (k - VIAL_USER) as usize;
            match SVALBOARD_VIAL_CUSTOM_KEYCODES.get(idx).and_then(|name| board.parse(name)) {
                Some(code) => code,
                None => {
                    report.unassigned_custom_keycodes += 1;
                    0
                }
            }
        };
        let used = |k: u16| k != 0 && k != 1;

        let layout = self
            .layout
            .iter()
            .map(|l| l.iter().map(|r| r.iter().map(|k| map(k.unwrap_or(0))).collect()).collect())
            .collect();
        let macros = self
            .macros
            .iter()
            .map(|m| {
                m.iter()
                    .map(|a| match a {
                        MacroAction::Tap(k) => MacroAction::Tap(map(*k)),
                        MacroAction::Down(k) => MacroAction::Down(map(*k)),
                        MacroAction::Up(k) => MacroAction::Up(map(*k)),
                        other => other.clone(),
                    })
                    .collect()
            })
            .collect();
        // Vial tap dances are always on; Vial combos have no flag or term of their own:
        // a combo with keys is on and uses the global combo term (0).
        let tap_dances = self
            .tap_dances
            .iter()
            .map(|t| TapDance {
                on_tap: map(t.on_tap),
                on_hold: map(t.on_hold),
                on_double_tap: map(t.on_double_tap),
                on_tap_hold: map(t.on_tap_hold),
                tapping_term: t.tapping_term & 0x7FFF,
                enabled: true,
            })
            .collect();
        let combos = self
            .combos
            .iter()
            .map(|c| {
                let keys = c.keys.map(&mut map);
                Combo { keys, output: map(c.output), options: if used(keys[0]) { COMBO_ENABLED } else { 0 } }
            })
            .collect();
        let key_overrides = self
            .key_overrides
            .iter()
            .map(|k| KeyOverride {
                trigger: map(k.trigger),
                replacement: map(k.replacement),
                layers: k.layers as u32,
                trigger_mods: k.trigger_mods,
                negative_mod_mask: k.negative_mod_mask,
                suppressed_mods: k.suppressed_mods,
                options: k.options,
            })
            .collect();
        let data = SvilData {
            uid: self.uid,
            svil_protocol: 0,
            via_protocol: self.via_protocol,
            keycode_version: crate::svil::KEYCODE_VERSION.to_string(),
            layout,
            macros,
            tap_dances,
            combos,
            key_overrides,
            settings: self.settings.clone(),
            layer_names: self.layer_names.clone(),
            ..Default::default()
        };
        (data, report)
    }
}

/// What a `.vil` → SVIL conversion changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MigrationReport {
    /// The file came from Svalboard's Vial firmware.
    pub svalboard: bool,
    /// `USERnn` positions Svalboard's Vial firmware left unassigned (now `KC_NO`).
    pub unassigned_custom_keycodes: usize,
    /// `USERnn` keycodes from another board's Vial firmware (kept, but meaningless here).
    pub foreign_custom_keycodes: usize,
}

/// Whether a JSON layout file is a Vial `.vil` (rather than a `.svil`).
pub fn is_vil(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.as_object().map(|o| o.contains_key("vial_protocol") && o.contains_key("layout")))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svil::keycode_names::CustomKeycode;

    fn sample() -> VilData {
        VilData {
            uid: SVALBOARD_VIAL_UID,
            vial_protocol: 6,
            via_protocol: 9,
            layout: vec![vec![vec![Some(0x04), Some(0x7E0C)], vec![None, Some(0x5221)]]],
            encoder_layout: vec![vec![(0x80, 0x81)]],
            layout_options: -1,
            macros: vec![vec![MacroAction::Tap(0x7E00), MacroAction::Text("x".into())]],
            tap_dances: vec![TapDanceEntry { on_tap: 4, on_hold: 5, on_double_tap: 0, on_tap_hold: 0, tapping_term: 200 }],
            combos: vec![ComboEntry { keys: [4, 5, 0, 0], output: 6 }, ComboEntry { keys: [0; 4], output: 0 }],
            key_overrides: vec![VialKeyOverride { trigger: 4, replacement: 5, layers: 0xFFFF, options: 0x80, ..Default::default() }],
            settings: [(2u16, 1u32)].into_iter().collect(),
            layer_names: [(0usize, "Base".to_string())].into_iter().collect(),
        }
    }

    #[test]
    fn roundtrip() {
        let d = sample();
        let text = d.to_json();
        assert!(text.contains("\"uid\": 5199957870438586395"));
        assert!(text.contains("\"vial_protocol\": 6"));
        assert!(text.contains("-1"));
        assert!(text.contains("\"USER12\""));
        assert!(is_vil(&text));
        assert_eq!(VilData::from_json(&text).unwrap(), d);
    }

    #[test]
    fn migrate_svalboard() {
        let board = KeycodeNames::new(
            SVALBOARD_VIAL_CUSTOM_KEYCODES.iter().map(|n| CustomKeycode { name: n.to_string(), ..Default::default() }).collect(),
        );
        let (s, report) = sample().to_svil(&board);
        assert!(report.svalboard);
        assert_eq!(report.unassigned_custom_keycodes, 0);
        // USER12 = SV_SNIPER_2 = QK_USER + 12 on the SVIL board.
        assert_eq!(s.layout[0][0][1], 0x7E40 + 12);
        assert_eq!(s.layout[0][1][0], 0);
        assert_eq!(s.macros[0][0], MacroAction::Tap(0x7E40));
        assert!(s.tap_dances[0].enabled);
        assert_eq!(s.combos[0].options, COMBO_ENABLED);
        assert_eq!(s.combos[1].options, 0);
        assert_eq!(s.key_overrides[0].layers, 0xFFFF);
        assert_eq!(s.layer_names.get(&0).map(String::as_str), Some("Base"));
    }

    #[test]
    fn migrate_foreign() {
        let mut d = sample();
        d.uid = 1;
        let (s, report) = d.to_svil(&KeycodeNames::default());
        assert!(!report.svalboard);
        assert_eq!(report.foreign_custom_keycodes, 2);
        assert_eq!(s.layout[0][0][1], 0x7E0C);
    }

    #[test]
    fn dynamic_entries() {
        assert_eq!(parse_dynamic_counts(&[8, 16, 4, 0]), (8, 16, 4));
        assert_eq!(parse_dynamic_entry(&[0, 1, 2]), Some(&[1u8, 2][..]));
        assert_eq!(parse_dynamic_entry(&[1, 1, 2]), None);
        let ko = VialKeyOverride::from_bytes(&[4, 0, 5, 0, 0xFF, 0xFF, 1, 2, 3, 0x80]).unwrap();
        assert_eq!(ko.layers, 0xFFFF);
        assert_eq!(ko.options, 0x80);
    }
}
