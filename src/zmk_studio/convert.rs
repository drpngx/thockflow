//! Conversion between ZMK Studio protobuf data and KeymapData.

use std::collections::HashMap;

use crate::keymap::{KeymapData, Layer, PhysicalKey};
use zmk_studio_rust_proto::zmk_behaviors_proto::zmk::behaviors;
use zmk_studio_rust_proto::zmk_keymap_proto::zmk::keymap;

/// Cached behavior metadata from the keyboard.
#[derive(Clone, Default, Debug)]
pub struct BehaviorCache {
    /// behavior_id → display_name (e.g., 0 → "Key Press")
    pub id_to_name: HashMap<i32, String>,
    /// display_name → behavior_id
    pub name_to_id: HashMap<String, i32>,
    /// behavior_id → short label for binding strings (e.g., "kp", "mo")
    pub id_to_label: HashMap<i32, String>,
    /// short label → behavior_id
    pub label_to_id: HashMap<String, i32>,
}

impl BehaviorCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a behavior from GetBehaviorDetailsResponse.
    pub fn register_behavior(&mut self, detail: &behaviors::GetBehaviorDetailsResponse) {
        let id = detail.id as i32;
        let display_name = detail.display_name.clone();

        // Derive short label from display_name:
        // "Key Press" → "kp", "Momentary Layer" → "mo", "Toggle Layer" → "tog", etc.
        let label = derive_label(&display_name);

        self.id_to_name.insert(id, display_name.clone());
        self.name_to_id.insert(display_name, id);
        self.id_to_label.insert(id, label.clone());
        self.label_to_id.insert(label, id);
    }
}

/// Derive a ZMK-style behavior label from a display name.
fn derive_label(display_name: &str) -> String {
    // ZMK Studio display names map to labels:
    let known = [
        ("Key Press", "kp"),
        ("Momentary Layer", "mo"),
        ("To Layer", "to"),
        ("Toggle Layer", "tog"),
        ("Layer Tap", "lt"),
        ("Layer-Tap", "lt"),
        ("Mod Tap", "mt"),
        ("Mod-Tap", "mt"),
        ("Sticky Key", "sk"),
        ("Sticky Layer", "sl"),
        ("Transparent", "trans"),
        ("None", "none"),
        ("Bluetooth", "bt"),
        ("Output Selection", "out"),
        ("Reset", "sys_reset"),
        ("Bootloader", "bootloader"),
        ("Grave Escape", "gresc"),
        ("Grave/Escape", "gresc"),
        ("Caps Word", "caps_word"),
        ("Key Repeat", "key_repeat"),
        ("Macro", "macro"),
        ("Hold Tap", "ht"),
        ("Soft Off", "soft_off"),
        ("Ext Power", "ext_power"),
        ("RGB Underglow", "rgb_ug"),
        ("Underglow", "rgb_ug"),
        ("Backlight", "bl"),
        ("Studio Unlock", "studio_unlock"),
        ("Mouse Key Press", "mkp"),
        ("Mouse Move", "mmv"),
        ("Mouse Scroll", "msc"),
        ("ms", "msc"),
        ("mm", "mmv"),
        ("mk", "mkp"),
    ];

    for (name, label) in &known {
        if display_name.eq_ignore_ascii_case(name) || display_name.to_lowercase() == label.to_lowercase() {
            return label.to_string();
        }
    }

    // Handle Bluetooth profiles specifically (e.g. "Bluetooth 1" -> "bt_1")
    if display_name.to_lowercase().starts_with("bluetooth ") {
        if let Some(digit) = display_name.split_whitespace().last() {
            if digit.chars().all(|c| c.is_ascii_digit()) {
                return format!("bt_{}", digit);
            }
        }
    }

    // Fallback: lowercase first letters of each word
    let fallback = display_name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_lowercase();

    if fallback.is_empty() {
        display_name.to_lowercase().replace(' ', "_")
    } else {
        fallback
    }
}

/// Convert raw PhysicalLayouts response to Vec<PhysicalKey>.
pub fn parse_physical_layouts(layouts: &keymap::PhysicalLayouts) -> (u32, Vec<(String, Vec<PhysicalKey>)>) {
    let active_index = layouts.active_layout_index;

    let mut parsed_layouts = Vec::new();
    for layout_data in &layouts.layouts {
        let name = layout_data.name.clone();

        let mut keys = Vec::new();
        for key_data in &layout_data.keys {
            keys.push(PhysicalKey {
                x: key_data.x,
                y: key_data.y,
                width: key_data.width,
                height: key_data.height,
                rotation: key_data.r,
                rx: key_data.rx,
                ry: key_data.ry,
            });
        }
        parsed_layouts.push((name, keys));
    }

    (active_index, parsed_layouts)
}

/// Convert raw Keymap response to layers.
pub fn parse_keymap(keymap_data: &keymap::Keymap, _cache: &BehaviorCache) -> (Vec<ProtoLayer>, u32, u32) {
    let mut layers = Vec::new();
    for layer_data in &keymap_data.layers {
        let mut bindings = Vec::new();
        for binding_data in &layer_data.bindings {
            bindings.push(ProtoBinding {
                behavior_id: binding_data.behavior_id,
                param1: binding_data.param1,
                param2: binding_data.param2,
            });
        }

        layers.push(ProtoLayer { id: layer_data.id, name: layer_data.name.clone(), bindings });
    }

    (layers, keymap_data.available_layers, keymap_data.max_layer_name_length)
}

#[derive(Clone, Debug)]
pub struct ProtoBinding {
    pub behavior_id: i32,
    pub param1: u32,
    pub param2: u32,
}

#[derive(Clone, Debug)]
pub struct ProtoLayer {
    pub id: u32,
    pub name: String,
    pub bindings: Vec<ProtoBinding>,
}

/// Convert proto layers + physical layout into KeymapData for the renderer.
pub fn to_keymap_data(
    physical_keys: Vec<PhysicalKey>,
    proto_layers: &[ProtoLayer],
    cache: &BehaviorCache,
) -> KeymapData {
    let layers = proto_layers
        .iter()
        .map(|l| Layer {
            name: if l.name.is_empty() { "base".to_string() } else { l.name.clone() },
            bindings: l
                .bindings
                .iter()
                .map(|b| format_binding(b, cache))
                .collect(),
        })
        .collect();

    KeymapData {
        layers,
        physical_layout: physical_keys,
        aliases: std::collections::HashMap::new(),
        defsrc: vec![],
        includes: vec![],
        unmapped_names: vec![],
    }
}

fn format_binding(binding: &ProtoBinding, cache: &BehaviorCache) -> String {
    let mut label = cache
        .id_to_label
        .get(&binding.behavior_id)
        .cloned()
        .unwrap_or_else(|| binding.behavior_id.to_string());

    // Normalize labels for formatting
    if label == "ms" { label = "msc".to_string(); }
    if label == "mm" { label = "mmv".to_string(); }
    if label == "mk" { label = "mkp".to_string(); }

    if label == "trans" || label == "none" {
        return format!("&{}", label);
    }

    let params_str = format_params(&label, binding.param1, binding.param2);
    if params_str.is_empty() {
        format!("&{}", label)
    } else {
        format!("&{} {}", label, params_str)
    }
}

fn format_params(label: &str, param1: u32, param2: u32) -> String {
    // Map usage IDs to common ZMK strings for readability
    fn usage_to_key(u: u32) -> String {
        let page = (u >> 16) & 0xFFFF;
        let id = u & 0xFFFF;

        // ZMK often uses page 0x07 for Keyboard/Keypad HID usage
        if page == 0x07 || (page == 0 && id > 0 && id < 0xE8) {
            match id {
                0x04 => "A", 0x05 => "B", 0x06 => "C", 0x07 => "D",
                0x08 => "E", 0x09 => "F", 0x0A => "G", 0x0B => "H",
                0x0C => "I", 0x0D => "J", 0x0E => "K", 0x0F => "L",
                0x10 => "M", 0x11 => "N", 0x12 => "O", 0x13 => "P",
                0x14 => "Q", 0x15 => "R", 0x16 => "S", 0x17 => "T",
                0x18 => "U", 0x19 => "V", 0x1A => "W", 0x1B => "X",
                0x1C => "Y", 0x1D => "Z",
                0x1E => "N1", 0x1F => "N2", 0x20 => "N3", 0x21 => "N4",
                0x22 => "N5", 0x23 => "N6", 0x24 => "N7", 0x25 => "N8",
                0x26 => "N9", 0x27 => "N0",
                0x28 => "RET", 0x29 => "ESC", 0x2A => "BSPC", 0x2B => "TAB",
                0x2C => "SPC", 0x2D => "MINUS", 0x2E => "EQUAL", 0x2F | 0xB6 => "LBKT",
                0x30 | 0xB7 => "RBKT", 0x31 => "BSLH", 0x33 => "SEMI", 0x34 => "SQT",
                0x35 => "GRAVE", 0x36 => "COMMA", 0x37 => "DOT", 0x38 => "FSLH",
                0x39 => "CAPS",
                0x3A => "F1", 0x3B => "F2", 0x3C => "F3", 0x3D => "F4",
                0x3E => "F5", 0x3F => "F6", 0x40 => "F7", 0x41 => "F8",
                0x42 => "F9", 0x43 => "F10", 0x44 => "F11", 0x45 => "F12",
                0x46 => "PSCRN", 0x47 => "SLCK", 0x48 => "PAUSE_BREAK",
                0x49 => "INS", 0x4A => "HOME", 0x4B => "PG_UP",
                0x4C => "DEL", 0x4D => "END", 0x4E => "PG_DN",
                0x4F => "RIGHT", 0x50 => "LEFT", 0x51 => "DOWN", 0x52 => "UP",
                0x53 => "KP_NUM", 0x54 => "KP_DIVIDE", 0x55 => "KP_MULTIPLY",
                0x56 => "KP_MINUS", 0x57 => "KP_PLUS", 0x58 => "KP_ENTER",
                0x59 => "KP_N1", 0x5A => "KP_N2", 0x5B => "KP_N3",
                0x5C => "KP_N4", 0x5D => "KP_N5", 0x5E => "KP_N6",
                0x5F => "KP_N7", 0x60 => "KP_N8", 0x61 => "KP_N9",
                0x62 => "KP_N0", 0x63 => "KP_DOT",
                0x65 => "K_APP",
                0xE0 => "LCTRL", 0xE1 => "LSHFT", 0xE2 => "LALT", 0xE3 => "LGUI",
                0xE4 => "RCTRL", 0xE5 => "RSHFT", 0xE6 => "RALT", 0xE7 => "RGUI",
                _ => return format!("0x{:02X}", id),
            }.to_string()
        } else if page == 0x0C {
            match id {
                0x30 => "C_POWER",
                0x31 => "C_RESET",
                0x32 => "C_SLEEP",
                0x40 => "C_MENU",
                0x6F => "C_BRIGHTNESS_INC",
                0x70 => "C_BRIGHTNESS_DEC",
                0xB5 => "C_NEXT",
                0xB6 => "C_PREV",
                0xB7 => "C_STOP",
                0xB8 => "C_EJECT",
                0xCD => "C_PP",
                0xE2 => "C_MUTE",
                0xE9 => "C_VOL_UP",
                0xEA => "C_VOL_DN",
                _ => return format!("C_0x{:02X}", id),
            }.to_string()
        } else {
            format!("0x{:08X}", u)
        }
    }

    fn pointing_constant(label: &str, p: u32) -> String {
        match label {
            "mkp" | "mk" => {
                match p {
                    1 => "LCLK".to_string(),
                    2 => "RCLK".to_string(),
                    4 => "MCLK".to_string(),
                    8 => "MB4".to_string(),
                    16 => "MB5".to_string(),
                    _ => p.to_string(),
                }
            }
            "mmv" | "mm" => {
                // Decode from 32-bit: X is top 16 bits, Y is bottom 16 bits
                let p32 = p as u32;
                let hor = ((p32 & 0xFFFF0000) >> 16) as i16;
                let vert = (p32 & 0x0000FFFF) as i16;
                
                if hor == -600 && vert == 0 {
                    "MOVE_LEFT".to_string()
                } else if hor == 600 && vert == 0 {
                    "MOVE_RIGHT".to_string()
                } else if hor == 0 && vert == -600 {
                    "MOVE_UP".to_string()
                } else if hor == 0 && vert == 600 {
                    "MOVE_DOWN".to_string()
                } else {
                    p.to_string() // Fallback if it doesn't match default macros perfectly
                }
            }
            "msc" | "ms" => {
                let p32 = p as u32;
                let hor = ((p32 & 0xFFFF0000) >> 16) as i16;
                let vert = (p32 & 0x0000FFFF) as i16;
                
                if hor == -10 && vert == 0 {
                    "SCRL_LEFT".to_string()
                } else if hor == 10 && vert == 0 {
                    "SCRL_RIGHT".to_string()
                } else if hor == 0 && vert == 10 {
                    "SCRL_UP".to_string()
                } else if hor == 0 && vert == -10 {
                    "SCRL_DOWN".to_string()
                } else {
                    p.to_string()
                }
            }
            _ => p.to_string(),
        }
    }

    match label {
        "kp" | "sk" => usage_to_key(param1),
        "mo" | "to" | "tog" | "sl" => param1.to_string(),
        "lt" => format!("{} {}", param1, usage_to_key(param2)),
        "mt" => format!("{} {}", usage_to_key(param1), usage_to_key(param2)),
        "mkp" | "msc" | "mmv" | "mk" | "ms" | "mm" => {
            if param2 != 0 {
                format!("{} {}", pointing_constant(label, param1), pointing_constant(label, param2))
            } else {
                pointing_constant(label, param1)
            }
        }
        "rgb_ug" => {
            let cmd = match param1 {
                0 => "TOG",
                1 => "ON",
                2 => "OFF",
                3 => "HUI",
                4 => "HUD",
                5 => "SAI",
                6 => "SAD",
                7 => "BRI",
                8 => "BRD",
                9 => "SPI",
                10 => "SPD",
                11 => "EFF",
                12 => "EFR",
                13 => "COLOR_HSV",
                _ => return format!("{} {}", param1, param2),
            };
            if param1 == 13 {
                format!("{} 0x{:06X}", cmd, param2)
            } else {
                cmd.to_string()
            }
        }
        _ => format!("{} {}", param1, param2),
    }
}

/// Parse a ZMK binding string back into proto binding components.
/// Returns (behavior_id, param1, param2) or None if parsing fails.
pub fn string_to_binding(
    binding_str: &str,
    cache: &BehaviorCache,
) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = binding_str.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let behavior_raw = parts[0];
    let label = behavior_raw.strip_prefix('&').unwrap_or(behavior_raw);
    let behavior_id = cache.label_to_id.get(label).copied()?;

    let params = &parts[1..];

    if label == "trans" || label == "none" {
        return Some((behavior_id, 0, 0));
    }

    if label == "kp" || label == "sk" {
        let param1 = params.get(0).map(|&s| keycode_to_hid_usage(s)).unwrap_or(0);
        return Some((behavior_id, param1, 0));
    }

    if label == "mo" || label == "to" || label == "tog" || label == "sl" {
        let param1 = params.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
        return Some((behavior_id, param1, 0));
    }

    if label == "lt" {
        let param1 = params.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
        let param2 = params.get(1).map(|&s| keycode_to_hid_usage(s)).unwrap_or(0);
        return Some((behavior_id, param1, param2));
    }

    if label == "mt" {
        let param1 = params.get(0).map(|&s| modifier_to_hid_usage(s)).unwrap_or(0);
        let param2 = params.get(1).map(|&s| keycode_to_hid_usage(s)).unwrap_or(0);
        return Some((behavior_id, param1, param2));
    }

    if label == "msc" || label == "ms" || label == "mmv" || label == "mm" || label == "mkp" || label == "mk" {
        let param1 = params.get(0).map(|&s| pointing_constant_to_val(label, s)).unwrap_or(0);
        let param2 = params.get(1).map(|&s| pointing_constant_to_val(label, s)).unwrap_or(0);
        return Some((behavior_id, param1, param2));
    }

    // Generic fallback
    let param1 = params.get(0).and_then(|s| parse_param(s)).unwrap_or(0);
    let param2 = params.get(1).and_then(|s| parse_param(s)).unwrap_or(0);
    Some((behavior_id, param1, param2))
}

fn pointing_constant_to_val(label: &str, s: &str) -> u32 {
    match label {
        "mkp" | "mk" => {
            match s {
                "LCLK" => 1,
                "RCLK" => 2,
                "MCLK" => 4,
                "MB4" => 8,
                "MB5" => 16,
                _ => s.parse::<u32>().unwrap_or(0),
            }
        }
        "mmv" | "mm" => {
            let (hor, vert): (i32, i32) = match s {
                "MOVE_UP" => (0, -600),
                "MOVE_DOWN" => (0, 600),
                "MOVE_LEFT" => (-600, 0),
                "MOVE_RIGHT" => (600, 0),
                _ => (0, s.parse::<i32>().unwrap_or(0)),
            };
            (((hor & 0xFFFF) << 16) | (vert & 0xFFFF)) as u32
        }
        "msc" | "ms" => {
            let (hor, vert): (i32, i32) = match s {
                "SCRL_UP" => (0, 10),
                "SCRL_DOWN" => (0, -10),
                "SCRL_LEFT" => (-10, 0),
                "SCRL_RIGHT" => (10, 0),
                _ => (0, s.parse::<i32>().unwrap_or(0)),
            };
            (((hor & 0xFFFF) << 16) | (vert & 0xFFFF)) as u32
        }
        _ => s.parse::<u32>().unwrap_or(0),
    }
}

fn parse_param(s: &str) -> Option<u32> {
    if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16).ok()
    } else {
        s.parse().ok()
    }
}

fn keycode_to_hid_usage(key: &str) -> u32 {
    let id = match key {
        "A" => 0x04, "B" => 0x05, "C" => 0x06, "D" => 0x07,
        "E" => 0x08, "F" => 0x09, "G" => 0x0A, "H" => 0x0B,
        "I" => 0x0C, "J" => 0x0D, "K" => 0x0E, "L" => 0x0F,
        "M" => 0x10, "N" => 0x11, "O" => 0x12, "P" => 0x13,
        "Q" => 0x14, "R" => 0x15, "S" => 0x16, "T" => 0x17,
        "U" => 0x18, "V" => 0x19, "W" => 0x1A, "X" => 0x1B,
        "Y" => 0x1C, "Z" => 0x1D,
        "N1" => 0x1E, "N2" => 0x1F, "N3" => 0x20, "N4" => 0x21,
        "N5" => 0x22, "N6" => 0x23, "N7" => 0x24, "N8" => 0x25,
        "N9" => 0x26, "N0" => 0x27,
        "ENTER" | "RET" => 0x28, "ESC" => 0x29, "BSPC" => 0x2A, "TAB" => 0x2B,
        "SPACE" | "SPC" => 0x2C, "MINUS" => 0x2D, "EQUAL" => 0x2E, "LBKT" => 0x2F,
        "RBKT" => 0x30, "BSLH" => 0x31, "SEMI" => 0x33, "SQT" | "APOS" => 0x34,
        "GRAVE" => 0x35, "COMMA" => 0x36, "DOT" => 0x37, "SLASH" | "FSLH" => 0x38,
        "CAPS" => 0x39,
        "F1" => 0x3A, "F2" => 0x3B, "F3" => 0x3C, "F4" => 0x3D,
        "F5" => 0x3E, "F6" => 0x3F, "F7" => 0x40, "F8" => 0x41,
        "F9" => 0x42, "F10" => 0x43, "F11" => 0x44, "F12" => 0x45,
        "PSCRN" => 0x46, "SLCK" => 0x47, "PAUSE_BREAK" => 0x48,
        "INS" => 0x49, "HOME" => 0x4A, "PG_UP" => 0x4B,
        "DEL" => 0x4C, "END" => 0x4D, "PG_DN" => 0x4E,
        "RIGHT" => 0x4F, "LEFT" => 0x50, "DOWN" => 0x51, "UP" => 0x52,
        "KP_NUM" => 0x53, "KP_DIVIDE" => 0x54, "KP_MULTIPLY" => 0x55,
        "KP_MINUS" => 0x56, "KP_PLUS" => 0x57, "KP_ENTER" => 0x58,
        "KP_N1" => 0x59, "KP_N2" => 0x5A, "KP_N3" => 0x5B,
        "KP_N4" => 0x5C, "KP_N5" => 0x5D, "KP_N6" => 0x5E,
        "KP_N7" => 0x5F, "KP_N8" => 0x60, "KP_N9" => 0x61,
        "KP_N0" => 0x62, "KP_DOT" => 0x63,
        "K_APP" => 0x65,
        "LCTRL" => 0xE0, "LSHFT" | "LSHIFT" => 0xE1, "LALT" => 0xE2, "LGUI" => 0xE3,
        "RCTRL" => 0xE4, "RSHFT" | "RSHIFT" => 0xE5, "RALT" => 0xE6, "RGUI" => 0xE7,
        _ => {
            if key.starts_with("0x") || key.starts_with("0X") {
                u32::from_str_radix(&key[2..], 16).unwrap_or(0)
            } else {
                0
            }
        }
    };
    
    if id > 0 && id <= 0xE7 {
        // Encode with HID Usage Page 0x07 (Keyboard/Keypad)
        0x00070000 | id
    } else {
        id
    }
}

fn modifier_to_hid_usage(modifier: &str) -> u32 {
    keycode_to_hid_usage(modifier)
}
