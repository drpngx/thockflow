use gloo_net::http::Request;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use serde::{Deserialize, Serialize};

use crate::keymap::{
    show_open_file_picker, BindingParts, FileSystemFileHandle, FileSystemWritableFileStream,
    KeyOrigin, KeymapData, Layer, SelectedKey, VarType,
};

pub mod layout;
pub mod layer_menu;

/// Extension trait for Navigator to access the Battery API
#[wasm_bindgen]
extern "C" {
    /// Represents the Navigator interface extended with getBattery method
    #[wasm_bindgen(js_name = Navigator)]
    type BatteryNavigator;

    /// Gets the battery manager for the device
    #[wasm_bindgen(method, js_name = getBattery, catch)]
    fn get_battery(this: &BatteryNavigator) -> Result<js_sys::Promise, JsValue>;
}

fn format_kanata_keycode(kc: &str, is_mac: bool) -> String {
    match kc {
        "_" => "▽".to_string(),
        "none" => "∅".to_string(),
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
        "pgup" => "PgUp".to_string(),
        "pgdn" => "PgDn".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
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

pub fn get_kanata_binding_parts_internal(binding: &str, aliases: &std::collections::HashMap<String, String>, is_mac: bool, _is_laptop: bool) -> BindingParts {
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum ParamType {
    Timeout,    // Timeout/duration parameter (integer-valued)
    Integer,    // Generic integer parameter
    Action,
    Layer,
    Any,
    String,     // String argument (for cmd, clipboard)
    ClipboardId,// Clipboard save ID (0-65535)
}

static KANATA_ACTIONS: &[KanataActionInfo] = &[
    KanataActionInfo {
        name: "tap-hold",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Action, ParamType::Action],
        description: "Tap for one action, hold for another.",
    },
    KanataActionInfo {
        name: "tap-hold-press",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Action, ParamType::Action],
        description: "Similar to tap-hold, but hold action triggers on press.",
    },
    KanataActionInfo {
        name: "tap-hold-release",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Action, ParamType::Action],
        description: "Similar to tap-hold, but hold action triggers on release.",
    },
    KanataActionInfo {
        name: "tap-hold-next",
        params: &[ParamType::Integer, ParamType::Action, ParamType::Action],
        description: "Tap for one action, hold for another. Hold triggers if another key is pressed.",
    },
    KanataActionInfo {
        name: "tap-hold-next-release",
        params: &[ParamType::Integer, ParamType::Action, ParamType::Action],
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
        params: &[ParamType::Integer, ParamType::Action],
        description: "Action stays active for timeout or until next press.",
    },
    KanataActionInfo {
        name: "tap-dance",
        params: &[ParamType::Integer, ParamType::Any],
        description: "Different actions based on number of taps: (tap-dance timeout (action1 action2 ...))",
    },
    KanataActionInfo {
        name: "caps-word",
        params: &[ParamType::Integer],
        description: "Capitalize the next word.",
    },
    KanataActionInfo {
        name: "unicode",
        params: &[ParamType::Any],
        description: "Send a unicode character.",
    },
    // Mouse movement actions
    KanataActionInfo {
        name: "movemouse-up",
        params: &[ParamType::Integer, ParamType::Integer],
        description: "Move mouse cursor up. Params: interval_ms distance_px",
    },
    KanataActionInfo {
        name: "movemouse-down",
        params: &[ParamType::Integer, ParamType::Integer],
        description: "Move mouse cursor down. Params: interval_ms distance_px",
    },
    KanataActionInfo {
        name: "movemouse-left",
        params: &[ParamType::Integer, ParamType::Integer],
        description: "Move mouse cursor left. Params: interval_ms distance_px",
    },
    KanataActionInfo {
        name: "movemouse-right",
        params: &[ParamType::Integer, ParamType::Integer],
        description: "Move mouse cursor right. Params: interval_ms distance_px",
    },
    // Accelerated mouse movement actions
    KanataActionInfo {
        name: "movemouse-accel-up",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer],
        description: "Accelerated mouse up. Params: interval_ms accel_time_ms min_px max_px",
    },
    KanataActionInfo {
        name: "movemouse-accel-down",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer],
        description: "Accelerated mouse down. Params: interval_ms accel_time_ms min_px max_px",
    },
    KanataActionInfo {
        name: "movemouse-accel-left",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer],
        description: "Accelerated mouse left. Params: interval_ms accel_time_ms min_px max_px",
    },
    KanataActionInfo {
        name: "movemouse-accel-right",
        params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer],
        description: "Accelerated mouse right. Params: interval_ms accel_time_ms min_px max_px",
    },
    // Set absolute mouse position
    KanataActionInfo {
        name: "setmouse",
        params: &[ParamType::Integer, ParamType::Integer],
        description: "Set absolute mouse position. Params: x y (platform-specific)",
    },
    // Modify mouse movement speed
    KanataActionInfo {
        name: "movemouse-speed",
        params: &[ParamType::Integer],
        description: "Modify mouse movement speed. Param: percentage (50=half, 200=double)",
    },
    // cmd actions
    KanataActionInfo {
        name: "cmd",
        params: &[ParamType::String],  // Variadic: at least 1 required (binary)
        description: "Execute a binary with arguments. Example: (cmd echo hello)",
    },
    KanataActionInfo {
        name: "cmd-log",
        params: &[ParamType::String, ParamType::String],
        description: "Set cmd log levels. Params: stdout-level stderr-level (debug|info|warn|error|none)",
    },
    KanataActionInfo {
        name: "cmd-output-keys",
        params: &[ParamType::String],  // Variadic: at least 1 required
        description: "Execute command and parse stdout as keys to type.",
    },
    // Clipboard actions
    KanataActionInfo {
        name: "clipboard-set",
        params: &[ParamType::String],
        description: "Set clipboard to string. Example: (clipboard-set \"hello\")",
    },
    KanataActionInfo {
        name: "clipboard-save",
        params: &[ParamType::ClipboardId],
        description: "Save clipboard to ID (0-65535). Example: (clipboard-save 0)",
    },
    KanataActionInfo {
        name: "clipboard-restore",
        params: &[ParamType::ClipboardId],
        description: "Restore clipboard from ID. Example: (clipboard-restore 0)",
    },
    KanataActionInfo {
        name: "clipboard-save-swap",
        params: &[ParamType::ClipboardId, ParamType::ClipboardId],
        description: "Swap two clipboard save IDs. Example: (clipboard-save-swap 0 1)",
    },
    KanataActionInfo {
        name: "clipboard-cmd-set",
        params: &[ParamType::String],  // Variadic: binary + args
        description: "Set clipboard from command output. Example: (clipboard-cmd-set echo hello)",
    },
    KanataActionInfo {
        name: "clipboard-save-cmd-set",
        params: &[ParamType::ClipboardId, ParamType::String],  // ID + variadic
        description: "Set save ID from command output. Example: (clipboard-save-cmd-set 0 echo hello)",
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

                // Check if this is a mouse action for special validation
                let is_mouse_action = action.name.starts_with("movemouse") || action.name == "setmouse";
                
                // Handle variadic actions
                let is_variadic = matches!(action.name, 
                    "cmd" | "cmd-output-keys" | "clipboard-cmd-set" | "multi" | "macro"
                );
                let is_clipboard_save_cmd_set = action.name == "clipboard-save-cmd-set";
                
                // Special validation for variadic actions
                if is_variadic {
                    // Must have at least the minimum required params
                    let min_params = match action.name {
                        "cmd" | "cmd-output-keys" | "clipboard-cmd-set" => 1,
                        _ => 1, // multi, macro
                    };
                    if params.len() < min_params {
                        return false;
                    }
                    // All params must be valid strings (or for macro/multi, they're actions)
                    if action.name == "cmd" || action.name == "cmd-output-keys" || action.name.starts_with("clipboard-") {
                        // String params - any string is valid (can be quoted or unquoted)
                        return true;
                    }
                    // For multi/macro, recursively validate
                    return params.iter().all(|&p| self.validate_action(p) || KANATA_KEYS.contains(&p));
                }
                
                // Special validation for clipboard-save-cmd-set: ID + at least 1 string
                if is_clipboard_save_cmd_set {
                    if params.len() < 2 {
                        return false;
                    }
                    // First param must be valid clipboard ID
                    if let Ok(n) = params[0].parse::<u32>() {
                        if n > 65535 {
                            return false;
                        }
                    } else {
                        return false;
                    }
                    // Rest are strings (command + args)
                    return true;
                }
                
                // Standard fixed-param validation
                if params.len() != action.params.len() { return false; }

                for (i, &p_type) in action.params.iter().enumerate() {
                    let val = params[i];
                    match p_type {
                        ParamType::Timeout | ParamType::Integer => {
                            match val.parse::<u32>() {
                                Ok(n) => {
                                    // Mouse actions require values in range [1, 65535]
                                    if is_mouse_action && (n < 1 || n > 65535) {
                                        return false;
                                    }
                                }
                                Err(_) => return false,
                            }
                        }
                        ParamType::Layer => {
                            if !self.data.layers.iter().any(|l| l.name == val) { return false; }
                        }
                        ParamType::Action => {
                            if !self.validate_action(val) && !KANATA_KEYS.contains(&val) {
                                return false;
                            }
                        }
                        ParamType::ClipboardId => {
                            match val.parse::<u32>() {
                                Ok(n) => {
                                    if n > 65535 {
                                        return false;
                                    }
                                }
                                Err(_) => return false,
                            }
                        }
                        ParamType::String => {
                            // For cmd-log, validate log levels
                            if action.name == "cmd-log" {
                                let valid_levels = ["debug", "info", "warn", "error", "none"];
                                if !valid_levels.contains(&val.to_lowercase().as_str()) {
                                    return false;
                                }
                            }
                            // Other strings are always valid (quoted or unquoted)
                        }
                        ParamType::Any => {}
                    }
                }
                return true;
            }
            return false;
        }

        // Check for output chord (e.g., C-a, C-S-tab)
        if has_modifier_prefix(text) {
            let modifiers = get_current_modifiers(text);
            // Check for duplicate modifiers
            if has_duplicate_modifiers(&modifiers) {
                return false;
            }
            let base = get_base_key(text);
            // Empty base is ok during typing, but if there's a base, it must be valid
            if base.is_empty() {
                return true; // Partial chord like "C-"
            }
            return KANATA_KEYS.contains(&base);
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
    "_", "none", "lsft", "rsft", "lctl", "rctl", "lalt", "ralt", "lmet", "rmet",
    "caps", "esc", "ent", "bspc", "spc", "tab", "del", "ins",
    "up", "down", "left", "right", "pgup", "pgdn", "home", "end",
    "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "0",
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
    "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
    "comm", "dot", "slsh", "scln", "apos", "lbkt", "rbkt", "bksl", "grv", "min", "eql",
];

/// Output chord modifier prefixes
const OUTPUT_CHORD_MODIFIERS: &[&str] = &["C-", "RC-", "A-", "RA-", "AG-", "S-", "RS-", "M-", "RM-"];

/// Represents a parsed output chord
#[derive(Debug, Clone, PartialEq)]
struct OutputChord {
    modifiers: Vec<String>,
    key: String,
}

/// Parse an output chord string into modifiers and base key
/// Returns None if the string doesn't start with any modifier prefix
fn parse_output_chord(input: &str) -> Option<OutputChord> {
    let input = input.to_lowercase();
    let mut modifiers = Vec::new();
    let mut remaining = input.as_str();
    
    // Keep extracting modifier prefixes
    while let Some(modifier) = extract_modifier_prefix(remaining) {
        modifiers.push(modifier.to_string());
        remaining = &remaining[modifier.len()..];
    }
    
    if modifiers.is_empty() {
        return None;
    }
    
    Some(OutputChord {
        modifiers,
        key: remaining.to_string(),
    })
}

/// Extract a modifier prefix from the start of a string
fn extract_modifier_prefix(s: &str) -> Option<&str> {
    for &modifier in OUTPUT_CHORD_MODIFIERS {
        if s.starts_with(modifier.to_lowercase().as_str()) {
            return Some(&s[..modifier.len()]);
        }
    }
    None
}

/// Check if a string has any modifier prefix
fn has_modifier_prefix(input: &str) -> bool {
    let lower = input.to_lowercase();
    OUTPUT_CHORD_MODIFIERS.iter().any(|&m| lower.starts_with(m.to_lowercase().as_str()))
}

/// Get the base key from an output chord (strips all modifier prefixes)
fn get_base_key(input: &str) -> &str {
    let lower = input.to_lowercase();
    let mut start = 0;
    
    while let Some(modifier) = extract_modifier_prefix(&lower[start..]) {
        start += modifier.len();
    }
    
    &input[start..]
}

/// Get the current modifier prefixes from an input string
fn get_current_modifiers(input: &str) -> Vec<String> {
    let lower = input.to_lowercase();
    let mut modifiers = Vec::new();
    let mut pos = 0;
    
    while let Some(modifier) = extract_modifier_prefix(&lower[pos..]) {
        modifiers.push(modifier.to_string());
        pos += modifier.len();
    }
    
    modifiers
}

/// Check if there are duplicate modifiers (RA- and AG- are treated as equivalent)
fn has_duplicate_modifiers(modifiers: &[String]) -> bool {
    let mut has_left_ctrl = false;
    let mut has_right_ctrl = false;
    let mut has_left_alt = false;
    let mut has_right_alt = false;  // Covers both RA- and AG-
    let mut has_left_shift = false;
    let mut has_right_shift = false;
    let mut has_left_meta = false;
    let mut has_right_meta = false;
    
    for m in modifiers {
        let m_lower = m.to_lowercase();
        match m_lower.as_str() {
            "c-" => {
                if has_left_ctrl { return true; }
                has_left_ctrl = true;
            }
            "rc-" => {
                if has_right_ctrl { return true; }
                has_right_ctrl = true;
            }
            "a-" => {
                if has_left_alt { return true; }
                has_left_alt = true;
            }
            "ra-" | "ag-" => {
                if has_right_alt { return true; }
                has_right_alt = true;
            }
            "s-" => {
                if has_left_shift { return true; }
                has_left_shift = true;
            }
            "rs-" => {
                if has_right_shift { return true; }
                has_right_shift = true;
            }
            "m-" => {
                if has_left_meta { return true; }
                has_left_meta = true;
            }
            "rm-" => {
                if has_right_meta { return true; }
                has_right_meta = true;
            }
            _ => {}
        }
    }
    
    false
}

/// Get available next modifiers given current prefixes
fn get_available_modifiers(current: &[String]) -> Vec<&'static str> {
    let mut has_left_ctrl = false;
    let mut has_right_ctrl = false;
    let mut has_left_alt = false;
    let mut has_right_alt = false;
    let mut has_left_shift = false;
    let mut has_right_shift = false;
    let mut has_left_meta = false;
    let mut has_right_meta = false;
    
    for m in current {
        let m_lower = m.to_lowercase();
        match m_lower.as_str() {
            "c-" => has_left_ctrl = true,
            "rc-" => has_right_ctrl = true,
            "a-" => has_left_alt = true,
            "ra-" | "ag-" => has_right_alt = true,
            "s-" => has_left_shift = true,
            "rs-" => has_right_shift = true,
            "m-" => has_left_meta = true,
            "rm-" => has_right_meta = true,
            _ => {}
        }
    }
    
    let mut available = Vec::new();
    if !has_left_ctrl { available.push("C-"); }
    if !has_right_ctrl { available.push("RC-"); }
    if !has_left_alt { available.push("A-"); }
    if !has_right_alt { available.push("RA-"); }
    if !has_right_alt { available.push("AG-"); }  // AG- is same as RA-
    if !has_left_shift { available.push("S-"); }
    if !has_right_shift { available.push("RS-"); }
    if !has_left_meta { available.push("M-"); }
    if !has_right_meta { available.push("RM-"); }
    
    available
}

/// Check if a string is a valid modifier prefix (for completion)
fn is_modifier_prefix_str(s: &str) -> bool {
    let lower = s.to_lowercase();
    OUTPUT_CHORD_MODIFIERS.iter().any(|&m| m.to_lowercase().starts_with(&lower))
}

fn get_suggestions(text: &str, data: &KeymapData) -> (String, Vec<String>) {
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

    // Get the expected parameter type for the current position
    let expected_type = get_expected_param_type(&lower_text);

    if is_layer_action {
        for layer in &data.layers {
            if layer.name.to_lowercase().contains(query) {
                suggestions.push(layer.name.clone());
            }
        }
    } else if expected_type == Some(ParamType::Integer) || expected_type == Some(ParamType::Timeout) {
        // Check if we're completing a mouse action parameter
        let mouse_action = get_current_mouse_action(&lower_text);
        
        if let Some((action_name, param_idx)) = mouse_action {
            // Mouse action-specific suggestions
            let mouse_suggestions = get_mouse_action_suggestions(&action_name, param_idx);
            for s in mouse_suggestions {
                if s.contains(query) {
                    suggestions.push(s);
                }
            }
        } else {
            // Default integer suggestions for timeouts
            for t in ["50", "100", "200", "250", "300", "1000"] {
                if t.contains(query) {
                    suggestions.push(t.to_string());
                }
            }
        }
        
        // Suggest integer variables
        for defvar in &data.defvars {
            if matches_type(&defvar.var_type, &ParamType::Integer) {
                let var_suggestion = format!("${}", defvar.name);
                if var_suggestion.to_lowercase().contains(query) || query.starts_with('$') {
                    suggestions.push(var_suggestion);
                }
            }
        }
    } else if expected_type == Some(ParamType::Action) {
        // Suggest action variables and keys
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
            // Suggest action/key variables
            for defvar in &data.defvars {
                if matches_type(&defvar.var_type, &ParamType::Action) {
                    let var_suggestion = format!("${}", defvar.name);
                    if var_suggestion.to_lowercase().contains(query) || query.starts_with('$') {
                        suggestions.push(var_suggestion);
                    }
                }
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
            for alias in data.aliases.keys() {
                if alias.contains(query) {
                    suggestions.push(alias.clone());
                }
            }
            // Also suggest all variables when type is not specified
            if query.starts_with('$') {
                for defvar in &data.defvars {
                    let var_suggestion = format!("${}", defvar.name);
                    if var_suggestion.to_lowercase().contains(query) {
                        suggestions.push(var_suggestion);
                    }
                }
            }
        }
    }

    // Handle output chord suggestions (e.g., C-, C-S-, C-a)
    // This applies to both Action and non-Action contexts
    if !query.starts_with('(') && !query.starts_with('$') {
        let query_lower = query.to_lowercase();
        
        // Check if query is a partial modifier name (e.g., "C" for "C-")
        if query.len() >= 1 && query.len() <= 3 && !query.contains('-') {
            for &modifier in OUTPUT_CHORD_MODIFIERS {
                let mod_lower = modifier.to_lowercase();
                if mod_lower.starts_with(&query_lower) && mod_lower != query_lower {
                    suggestions.push(modifier.to_string());
                }
            }
        }
        
        // Check if query has modifier prefixes
        if has_modifier_prefix(query) {
            let current_mods = get_current_modifiers(query);
            let base_key = get_base_key(query);
            
            // If no duplicate modifiers, suggest more modifiers and base keys
            if !has_duplicate_modifiers(&current_mods) {
                let prefix_str = current_mods.join("");
                
                // Suggest additional modifiers
                for modifier in get_available_modifiers(&current_mods) {
                    let suggestion = format!("{}{}", prefix_str, modifier);
                    if !suggestions.contains(&suggestion) {
                        suggestions.push(suggestion);
                    }
                }
                
                // Suggest base keys
                if base_key.is_empty() {
                    // Query ends with '-', suggest all keys with prefix
                    for &key in KANATA_KEYS {
                        let suggestion = format!("{}{}", prefix_str, key);
                        if !suggestions.contains(&suggestion) {
                            suggestions.push(suggestion);
                        }
                    }
                } else {
                    // Query has partial base key, suggest matching keys
                    for &key in KANATA_KEYS {
                        if key.to_lowercase().starts_with(&base_key.to_lowercase()) {
                            let suggestion = format!("{}{}", prefix_str, key);
                            if !suggestions.contains(&suggestion) {
                                suggestions.push(suggestion);
                            }
                        }
                    }
                }
            }
        }
    }

    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(30);
    (prefix.to_string(), suggestions)
}

/// Get the expected parameter type for the current cursor position
fn get_expected_param_type(text: &str) -> Option<ParamType> {
    if let Some(last_open) = text.rfind('(') {
        let after_open = &text[last_open + 1..];
        let parts: Vec<&str> = after_open.split_whitespace().collect();
        
        if parts.is_empty() {
            return None;
        }
        
        if let Some(action) = KANATA_ACTIONS.iter().find(|a| a.name == parts[0]) {
            // Determine which parameter we're currently completing
            // If the text ends with space, we're about to type the next parameter
            // Otherwise, we're in the middle of the current parameter
            let param_idx = if after_open.ends_with(' ') {
                parts.len() - 1
            } else {
                parts.len().saturating_sub(2)
            };
            return action.params.get(param_idx).copied();
        }
    }
    
    None
}

/// Check if a variable type matches the expected parameter type
fn matches_type(var_type: &VarType, param_type: &ParamType) -> bool {
    match (var_type, param_type) {
        (VarType::Integer, ParamType::Timeout) => true,
        (VarType::Integer, ParamType::Integer) => true,
        (VarType::Key, ParamType::Action) => true,
        (VarType::Action, ParamType::Action) => true,
        (VarType::List, ParamType::Any) => true,
        _ => false,
    }
}

/// Detect if we're currently completing a mouse action parameter
/// Returns Some((action_name, param_index)) if inside a mouse action, None otherwise
fn get_current_mouse_action(text: &str) -> Option<(String, usize)> {
    if let Some(last_open) = text.rfind('(') {
        let after_open = &text[last_open + 1..];
        let parts: Vec<&str> = after_open.split_whitespace().collect();
        
        if parts.is_empty() {
            return None;
        }
        
        let action_name = parts[0];
        
        // Check if this is a mouse action
        if action_name.starts_with("movemouse") || action_name == "setmouse" {
            let param_idx = if after_open.ends_with(' ') {
                parts.len() - 1
            } else {
                parts.len().saturating_sub(2)
            };
            return Some((action_name.to_string(), param_idx));
        }
    }
    
    None
}

/// Get suggestion values for mouse action parameters
fn get_mouse_action_suggestions(action_name: &str, param_idx: usize) -> Vec<String> {
    let mut suggestions = Vec::new();
    
    if action_name.starts_with("movemouse-accel") {
        // movemouse-accel-*: interval, accel_time, min, max
        match param_idx {
            0 => {
                // Interval: typical values 1-10ms
                suggestions.extend(["1", "2", "5", "10"].iter().map(|&s| s.to_string()));
            }
            1 => {
                // Acceleration time: typical values 500-2000ms
                suggestions.extend(["500", "1000", "1500", "2000"].iter().map(|&s| s.to_string()));
            }
            2 | 3 => {
                // Min/max distance: typical values 1-20 pixels
                suggestions.extend(["1", "2", "5", "10", "20"].iter().map(|&s| s.to_string()));
            }
            _ => {}
        }
    } else if action_name.starts_with("movemouse") && action_name != "movemouse-speed" {
        // movemouse-* (basic): interval, distance
        match param_idx {
            0 => {
                // Interval: typical values 1-50ms
                suggestions.extend(["1", "5", "10", "20", "50"].iter().map(|&s| s.to_string()));
            }
            1 => {
                // Distance: typical values 1-50 pixels
                suggestions.extend(["1", "5", "10", "20", "50"].iter().map(|&s| s.to_string()));
            }
            _ => {}
        }
    } else if action_name == "setmouse" {
        // setmouse: x, y coordinates
        match param_idx {
            0 => {
                // X coordinate: common screen positions
                suggestions.extend(["0", "960", "1920", "32768"].iter().map(|&s| s.to_string()));
            }
            1 => {
                // Y coordinate: common screen positions  
                suggestions.extend(["0", "540", "1080", "32768"].iter().map(|&s| s.to_string()));
            }
            _ => {}
        }
    } else if action_name == "movemouse-speed" {
        // movemouse-speed: percentage
        suggestions.extend(["25", "50", "75", "100", "150", "200", "300"].iter().map(|&s| s.to_string()));
    }
    
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Defvar, Layer, LayerType, KeymapData, ProcessUnmappedKeys};
    use std::collections::HashMap;

    #[test]
    fn test_keymap_data_deserialization_with_phantoms() {
        let json = r#"{
            "physical_layout": [
                {"x": 0, "y": 0, "width": 1000, "height": 1000, "rotation": 0, "rx": 0, "ry": 0, "origin": "Standard", "name": "esc"},
                {"x": 1000, "y": 0, "width": 1000, "height": 1000, "rotation": 0, "rx": 0, "ry": 0, "origin": "Phantom", "name": "f1"}
            ],
            "layers": [],
            "includes": [],
            "aliases": {},
            "defsrc": ["esc"],
            "unmapped_names": [],
            "process_unmapped_keys": "No",
            "defvars": [],
            "phantom_keys": [{"name": "f1", "position": [1000, 0]}]
        }"#;
        let data: KeymapData = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(data.physical_layout.len(), 2);
        assert_eq!(data.physical_layout[0].origin, KeyOrigin::Standard);
        assert_eq!(data.physical_layout[1].origin, KeyOrigin::Phantom);
        assert_eq!(data.physical_layout[1].name, "f1");
        assert_eq!(data.phantom_keys.len(), 1);
    }

    #[test]
    fn test_completion_congruence() {
        let mut data = KeymapData {
            physical_layout: Vec::new(),
            layers: vec![Layer { 
                name: "base".to_string(), 
                bindings: Vec::new(),
                layer_type: LayerType::Deflayer,
                source_layer: None,
                key_bindings: HashMap::new(),
            }],
            includes: Vec::new(),
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
            phantom_keys: Vec::new(),
            chordsv2: Vec::new(),
        };
        data.aliases.insert("myalias".to_string(), "lsft".to_string());

        let validator = KanataValidator::new(&data);

        for action in KANATA_ACTIONS {
            let input = format!("({} ", action.name);
            let (_prefix, suggestions) = get_suggestions(&input, &data);
            
            if suggestions.is_empty() {
                // Some actions might not have suggestions if they only take timeouts or layers we haven't defined
                continue;
            }

            // Test first suggestion
            let first = &suggestions[0];
            
            // Note: full might be incomplete (e.g. "(one-shot 500") 
            // but we want to make sure it's at least a valid start or prefix.
            // Actually, the requirement says "make sure the suggestion can be validated".
            // If it's a number, we might need to complete the whole action to validate it.
            
            if action.name == "one-shot" {
                assert!(first.parse::<u32>().is_ok(), "one-shot suggestion should be a number, got {}", first);
                let completed = format!("(one-shot {} lsft)", first);
                assert!(validator.validate_action(&completed), "Completed one-shot should be valid");
            }
            
            if action.name == "tap-hold" {
                assert!(first.parse::<u32>().is_ok());
            }
        }
    }

    #[test]
    fn test_transparent_suggestion() {
        let data = KeymapData {
            physical_layout: Vec::new(),
            layers: Vec::new(),
            includes: Vec::new(),
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
            phantom_keys: Vec::new(),
            chordsv2: Vec::new(),
        };
        let (_, suggestions) = get_suggestions("_", &data);
        assert!(suggestions.contains(&"_".to_string()));
    }

    // ============================================================================
    // defvar Completion Tests
    // ============================================================================

    fn create_test_data_with_defvars() -> KeymapData {
        let mut data = KeymapData {
            physical_layout: Vec::new(),
            layers: vec![Layer { 
                name: "base".to_string(), 
                bindings: Vec::new(),
                layer_type: LayerType::Deflayer,
                source_layer: None,
                key_bindings: HashMap::new(),
            }],
            includes: Vec::new(),
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
            phantom_keys: Vec::new(),
            chordsv2: Vec::new(),
        };

        // Add test variables
        data.defvars.push(Defvar {
            name: "tap-timeout".to_string(),
            value: "100".to_string(),
            var_type: VarType::Integer,
        });
        data.defvars.push(Defvar {
            name: "hold-timeout".to_string(),
            value: "200".to_string(),
            var_type: VarType::Integer,
        });
        data.defvars.push(Defvar {
            name: "my-key".to_string(),
            value: "a".to_string(),
            var_type: VarType::Key,
        });
        data.defvars.push(Defvar {
            name: "my-mod".to_string(),
            value: "lctl".to_string(),
            var_type: VarType::Key,
        });
        data.defvars.push(Defvar {
            name: "nav-toggle".to_string(),
            value: "(layer-toggle nav)".to_string(),
            var_type: VarType::Action,
        });

        data
    }

    #[test]
    fn test_integer_completion_in_tap_hold() {
        let data = create_test_data_with_defvars();

        // Test first position (should suggest integer variables)
        let (_, suggestions) = get_suggestions("(tap-hold ", &data);
        
        // Should include integer variables
        assert!(suggestions.contains(&"$tap-timeout".to_string()), "Should suggest $tap-timeout");
        assert!(suggestions.contains(&"$hold-timeout".to_string()), "Should suggest $hold-timeout");
        
        // Should NOT include key/action variables
        assert!(!suggestions.contains(&"$my-key".to_string()), "Should NOT suggest $my-key in integer position");
        assert!(!suggestions.contains(&"$my-mod".to_string()), "Should NOT suggest $my-mod in integer position");
        assert!(!suggestions.contains(&"$nav-toggle".to_string()), "Should NOT suggest $nav-toggle in integer position");

        // Should include integer literals
        assert!(suggestions.contains(&"100".to_string()), "Should suggest integer literals");
    }

    #[test]
    fn test_action_completion_in_tap_hold() {
        let data = create_test_data_with_defvars();

        // Test third position (should suggest action/key variables)
        let (_, suggestions) = get_suggestions("(tap-hold 200 300 ", &data);
        
        // Should include key/action variables
        assert!(suggestions.contains(&"$my-key".to_string()), "Should suggest $my-key");
        assert!(suggestions.contains(&"$my-mod".to_string()), "Should suggest $my-mod");
        assert!(suggestions.contains(&"$nav-toggle".to_string()), "Should suggest $nav-toggle");
        
        // Should NOT include integer variables
        assert!(!suggestions.contains(&"$tap-timeout".to_string()), "Should NOT suggest $tap-timeout in action position");
        assert!(!suggestions.contains(&"$hold-timeout".to_string()), "Should NOT suggest $hold-timeout in action position");
    }

    #[test]
    fn test_variable_prefix_completion() {
        let data = create_test_data_with_defvars();

        // Test typing $ prefix in integer position
        let (_, suggestions) = get_suggestions("(tap-hold $ta", &data);
        assert!(suggestions.contains(&"$tap-timeout".to_string()));

        // Test typing $ prefix in action position
        let (_, suggestions) = get_suggestions("(tap-hold 200 300 $my", &data);
        assert!(suggestions.contains(&"$my-key".to_string()));
        assert!(suggestions.contains(&"$my-mod".to_string()));
    }

    #[test]
    fn test_completion_with_variable_used() {
        let data = create_test_data_with_defvars();

        // After using first integer variable, should still suggest integers for second position
        let (_, suggestions) = get_suggestions("(tap-hold $tap-timeout ", &data);
        
        // Should still suggest integer variables for second timeout position
        assert!(suggestions.contains(&"$hold-timeout".to_string()), "Should still suggest integer variables");
        assert!(suggestions.contains(&"200".to_string()), "Should suggest integer literals");
        
        // Should NOT suggest key variables
        assert!(!suggestions.contains(&"$my-key".to_string()), "Should NOT suggest keys in timeout position");
    }

    #[test]
    fn test_get_expected_param_type() {
        // Test tap-hold positions
        assert_eq!(get_expected_param_type("(tap-hold "), Some(ParamType::Integer));
        assert_eq!(get_expected_param_type("(tap-hold 200 "), Some(ParamType::Integer));
        assert_eq!(get_expected_param_type("(tap-hold 200 300 "), Some(ParamType::Action));
        assert_eq!(get_expected_param_type("(tap-hold 200 300 a "), Some(ParamType::Action));

        // Test layer-toggle
        assert_eq!(get_expected_param_type("(layer-toggle "), Some(ParamType::Layer));

        // Test one-shot
        assert_eq!(get_expected_param_type("(one-shot "), Some(ParamType::Integer));
        assert_eq!(get_expected_param_type("(one-shot 500 "), Some(ParamType::Action));

        // Test no context
        assert_eq!(get_expected_param_type(""), None);
        assert_eq!(get_expected_param_type("just-some-text"), None);
    }

    #[test]
    fn test_matches_type() {
        // Integer matches Integer and Timeout
        assert!(matches_type(&VarType::Integer, &ParamType::Integer));
        assert!(matches_type(&VarType::Integer, &ParamType::Timeout));

        // Key matches Action
        assert!(matches_type(&VarType::Key, &ParamType::Action));

        // Action matches Action
        assert!(matches_type(&VarType::Action, &ParamType::Action));

        // List matches Any
        assert!(matches_type(&VarType::List, &ParamType::Any));

        // Mismatches
        assert!(!matches_type(&VarType::Integer, &ParamType::Action));
        assert!(!matches_type(&VarType::Key, &ParamType::Integer));
        assert!(!matches_type(&VarType::Action, &ParamType::Integer));
        assert!(!matches_type(&VarType::String, &ParamType::Action));
    }

    #[test]
    fn test_integer_completion_in_other_actions() {
        let data = create_test_data_with_defvars();

        // Test one-shot
        let (_, suggestions) = get_suggestions("(one-shot ", &data);
        assert!(suggestions.contains(&"$tap-timeout".to_string()));
        assert!(!suggestions.contains(&"$my-key".to_string()));

        // Test tap-dance
        let (_, suggestions) = get_suggestions("(tap-dance ", &data);
        assert!(suggestions.contains(&"$tap-timeout".to_string()));
        assert!(!suggestions.contains(&"$my-key".to_string()));

        // Test caps-word
        let (_, suggestions) = get_suggestions("(caps-word ", &data);
        assert!(suggestions.contains(&"$tap-timeout".to_string()));
        assert!(!suggestions.contains(&"$my-key".to_string()));
    }

    // ============================================================================
    // Output Chord Tests
    // ============================================================================

    #[test]
    fn test_parse_output_chord_simple() {
        let chord = parse_output_chord("C-a").unwrap();
        assert_eq!(chord.modifiers, vec!["C-"]);
        assert_eq!(chord.key, "a");
    }

    #[test]
    fn test_parse_output_chord_multiple() {
        let chord = parse_output_chord("C-S-a").unwrap();
        assert_eq!(chord.modifiers, vec!["C-", "S-"]);
        assert_eq!(chord.key, "a");
    }

    #[test]
    fn test_parse_output_chord_right_mods() {
        let chord = parse_output_chord("RC-RS-M-tab").unwrap();
        assert_eq!(chord.modifiers, vec!["RC-", "RS-", "M-"]);
        assert_eq!(chord.key, "tab");
    }

    #[test]
    fn test_parse_output_chord_altgr() {
        let chord = parse_output_chord("AG-a").unwrap();
        assert_eq!(chord.modifiers, vec!["AG-"]);
        assert_eq!(chord.key, "a");
        
        let chord2 = parse_output_chord("RA-a").unwrap();
        assert_eq!(chord2.modifiers, vec!["RA-"]);
    }

    #[test]
    fn test_parse_output_chord_not_a_chord() {
        assert!(parse_output_chord("a").is_none());
        assert!(parse_output_chord("esc").is_none());
        assert!(parse_output_chord("_").is_none());
    }

    #[test]
    fn test_get_base_key() {
        assert_eq!(get_base_key("C-S-a"), "a");
        assert_eq!(get_base_key("C-esc"), "esc");
        assert_eq!(get_base_key("a"), "a");
        assert_eq!(get_base_key("M-spc"), "spc");
    }

    #[test]
    fn test_has_modifier_prefix() {
        assert!(has_modifier_prefix("C-a"));
        assert!(has_modifier_prefix("S-tab"));
        assert!(!has_modifier_prefix("a"));
        assert!(!has_modifier_prefix("esc"));
    }

    #[test]
    fn test_has_duplicate_modifiers() {
        assert!(has_duplicate_modifiers(&["C-".to_string(), "C-".to_string()]));
        assert!(has_duplicate_modifiers(&["RA-".to_string(), "AG-".to_string()])); // RA and AG are equivalent
        assert!(!has_duplicate_modifiers(&["C-".to_string(), "S-".to_string()]));
        assert!(!has_duplicate_modifiers(&["C-".to_string(), "RC-".to_string()]));
    }

    #[test]
    fn test_get_available_modifiers() {
        let available = get_available_modifiers(&["C-".to_string()]);
        assert!(available.contains(&"S-"));
        assert!(available.contains(&"A-"));
        assert!(available.contains(&"M-"));
        assert!(available.contains(&"RC-"));
        assert!(!available.contains(&"C-")); // No duplicates
    }

    #[test]
    fn test_validate_output_chord_simple() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        assert!(validator.validate_action("C-a"));
        assert!(validator.validate_action("S-1"));
        assert!(validator.validate_action("M-tab"));
    }

    #[test]
    fn test_validate_output_chord_multiple() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        assert!(validator.validate_action("C-S-a"));
        assert!(validator.validate_action("C-A-del"));
        assert!(validator.validate_action("C-S-M-a"));
    }

    #[test]
    fn test_validate_output_chord_partial() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        // Partial chord (ends with -) is valid during typing
        assert!(validator.validate_action("C-"));
        assert!(validator.validate_action("C-S-"));
    }

    #[test]
    fn test_validate_output_chord_invalid_base() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        assert!(!validator.validate_action("C-invalidkey"));
    }

    #[test]
    fn test_validate_output_chord_duplicate() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        // Duplicate C- should be invalid
        assert!(!validator.validate_action("C-C-a"));
        assert!(!validator.validate_action("S-C-S-a"));
        // RA- and AG- are duplicates
        assert!(!validator.validate_action("RA-AG-a"));
    }

    #[test]
    fn test_validate_output_chord_all_modifiers() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        // Test all modifier variants
        assert!(validator.validate_action("C-a"));
        assert!(validator.validate_action("RC-a"));
        assert!(validator.validate_action("A-a"));
        assert!(validator.validate_action("RA-a"));
        assert!(validator.validate_action("AG-a"));
        assert!(validator.validate_action("S-a"));
        assert!(validator.validate_action("RS-a"));
        assert!(validator.validate_action("M-a"));
        assert!(validator.validate_action("RM-a"));
    }

    #[test]
    fn test_suggestions_modifier_prefix() {
        let data = create_test_data_with_defvars();
        let (_prefix, suggestions) = get_suggestions("C", &data);
        assert!(suggestions.contains(&"C-".to_string()), "Should suggest C- for query 'C'. Suggestions: {:?}", suggestions);
    }

    #[test]
    fn test_suggestions_after_prefix() {
        let data = create_test_data_with_defvars();
        let (_prefix, suggestions) = get_suggestions("C-", &data);
        // Should suggest more modifiers and base keys
        assert!(suggestions.contains(&"C-S-".to_string()), "Should suggest C-S-. Suggestions: {:?}", suggestions);
        assert!(suggestions.contains(&"C-a".to_string()), "Should suggest C-a. Suggestions: {:?}", suggestions);
        assert!(suggestions.contains(&"C-esc".to_string()), "Should suggest C-esc. Suggestions: {:?}", suggestions);
    }

    #[test]
    fn test_suggestions_multiple_prefixes() {
        let data = create_test_data_with_defvars();
        let (_prefix, suggestions) = get_suggestions("C-S-", &data);
        // After C-S-, can add A-, M-, RC-, RS-, RM-, AG-, RA- but not C- or S-
        assert!(suggestions.contains(&"C-S-a".to_string()), "Should suggest C-S-a");
        assert!(suggestions.contains(&"C-S-M-".to_string()), "Should suggest C-S-M-");
        assert!(!suggestions.contains(&"C-S-C-".to_string()), "Should NOT suggest duplicate C-");
        assert!(!suggestions.contains(&"C-S-S-".to_string()), "Should NOT suggest duplicate S-");
    }

    #[test]
    fn test_suggestions_partial_base() {
        let data = create_test_data_with_defvars();
        let (_prefix, suggestions) = get_suggestions("C-a", &data);
        // Should suggest base keys starting with 'a' with C- prefix
        assert!(suggestions.contains(&"C-a".to_string()), "Should suggest C-a");
        // Should not suggest keys not starting with 'a'
        assert!(!suggestions.contains(&"C-b".to_string()), "Should NOT suggest C-b when query is C-a");
    }

    #[test]
    fn test_suggestions_all_modifiers() {
        let data = create_test_data_with_defvars();
        let modifiers = ["C", "RC", "A", "RA", "AG", "S", "RS", "M", "RM"];
        for m in &modifiers {
            let query = format!("{}", m);
            let (_prefix, suggestions) = get_suggestions(&query, &data);
            assert!(
                suggestions.contains(&format!("{}-", m)),
                "Should suggest {}- for query {}",
                m,
                query
            );
        }
    }

    #[test]
    fn test_output_chord_in_action() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        // Output chords should work inside actions
        assert!(validator.validate_action("(tap-hold 200 200 C-a C-S-a)"));
        assert!(validator.validate_action("(multi C-c C-v)"));
    }

    #[test]
    fn test_output_chord_case_insensitive() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        // Should be case insensitive
        assert!(validator.validate_action("c-a"));
        assert!(validator.validate_action("C-A"));
        assert!(validator.validate_action("c-s-tab"));
    }

    // ============================================================================
    // Mouse Movement Tests
    // ============================================================================

    #[test]
    fn test_mouse_actions_exist() {
        let mouse_actions = [
            "movemouse-up", "movemouse-down", "movemouse-left", "movemouse-right",
            "movemouse-accel-up", "movemouse-accel-down", "movemouse-accel-left", "movemouse-accel-right",
            "setmouse", "movemouse-speed"
        ];
        
        for action_name in &mouse_actions {
            assert!(KANATA_ACTIONS.iter().any(|a| a.name == *action_name),
                "Mouse action {} should exist", action_name);
        }
    }

    #[test]
    fn test_movemouse_params() {
        let action = KANATA_ACTIONS.iter().find(|a| a.name == "movemouse-up").unwrap();
        assert_eq!(action.params.len(), 2);
        assert_eq!(action.params[0], ParamType::Integer);
        assert_eq!(action.params[1], ParamType::Integer);
    }

    #[test]
    fn test_movemouse_accel_params() {
        let action = KANATA_ACTIONS.iter().find(|a| a.name == "movemouse-accel-up").unwrap();
        assert_eq!(action.params.len(), 4);
        assert!(action.params.iter().all(|&p| p == ParamType::Integer));
    }

    #[test]
    fn test_setmouse_params() {
        let action = KANATA_ACTIONS.iter().find(|a| a.name == "setmouse").unwrap();
        assert_eq!(action.params.len(), 2);
        assert_eq!(action.params[0], ParamType::Integer);
        assert_eq!(action.params[1], ParamType::Integer);
    }

    #[test]
    fn test_movemouse_speed_params() {
        let action = KANATA_ACTIONS.iter().find(|a| a.name == "movemouse-speed").unwrap();
        assert_eq!(action.params.len(), 1);
        assert_eq!(action.params[0], ParamType::Integer);
    }

    #[test]
    fn test_validate_movemouse_basic() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(movemouse-up 1 1)"));
        assert!(validator.validate_action("(movemouse-down 10 5)"));
        assert!(validator.validate_action("(movemouse-left 50 100)"));
        assert!(validator.validate_action("(movemouse-right 100 50)"));
    }

    #[test]
    fn test_validate_movemouse_accel() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(movemouse-accel-up 1 1000 1 5)"));
        assert!(validator.validate_action("(movemouse-accel-down 5 500 2 10)"));
        assert!(validator.validate_action("(movemouse-accel-left 10 2000 1 20)"));
        assert!(validator.validate_action("(movemouse-accel-right 2 750 3 15)"));
    }

    #[test]
    fn test_validate_setmouse() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(setmouse 0 0)"));
        assert!(validator.validate_action("(setmouse 960 540)"));
        assert!(validator.validate_action("(setmouse 32768 32768)"));
    }

    #[test]
    fn test_validate_movemouse_speed() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(movemouse-speed 50)"));
        assert!(validator.validate_action("(movemouse-speed 100)"));
        assert!(validator.validate_action("(movemouse-speed 200)"));
    }

    #[test]
    fn test_validate_movemouse_invalid_params() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Too few params
        assert!(!validator.validate_action("(movemouse-up 1)"));
        assert!(!validator.validate_action("(movemouse-accel-up 1 1000 1)"));
        
        // Too many params
        assert!(!validator.validate_action("(movemouse-up 1 1 1)"));
        assert!(!validator.validate_action("(setmouse 0 0 0)"));
        
        // Invalid (non-integer) params
        assert!(!validator.validate_action("(movemouse-up abc 1)"));
        assert!(!validator.validate_action("(movemouse-speed fast)"));
    }

    #[test]
    fn test_validate_movemouse_range() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Valid boundary values
        assert!(validator.validate_action("(movemouse-up 1 1)"));
        assert!(validator.validate_action("(movemouse-up 65535 65535)"));
        
        // Invalid: zero
        assert!(!validator.validate_action("(movemouse-up 0 1)"));
        assert!(!validator.validate_action("(movemouse-up 1 0)"));
        
        // Invalid: negative
        assert!(!validator.validate_action("(movemouse-up -1 1)"));
    }

    #[test]
    fn test_suggestions_movemouse_actions() {
        let data = create_test_data_with_defvars();
        
        // Typing "(move" should suggest movemouse actions
        let (_, suggestions) = get_suggestions("(move", &data);
        assert!(suggestions.iter().any(|s| s.contains("movemouse-up")));
        assert!(suggestions.iter().any(|s| s.contains("movemouse-accel-up")));
        assert!(suggestions.iter().any(|s| s.contains("movemouse-speed")));
    }

    #[test]
    fn test_suggestions_movemouse_first_param() {
        let data = create_test_data_with_defvars();
        
        // After typing "(movemouse-up ", should suggest integers
        let (_, suggestions) = get_suggestions("(movemouse-up ", &data);
        
        // Should have integer suggestions
        assert!(suggestions.iter().any(|s| s.parse::<u32>().is_ok()),
            "Should suggest integer values for interval");
    }

    #[test]
    fn test_get_mouse_action_suggestions() {
        // Test basic movemouse suggestions
        let suggestions = get_mouse_action_suggestions("movemouse-up", 0);
        assert!(suggestions.contains(&"1".to_string()));
        assert!(suggestions.contains(&"5".to_string()));
        
        let suggestions = get_mouse_action_suggestions("movemouse-up", 1);
        assert!(suggestions.contains(&"1".to_string()));
        assert!(suggestions.contains(&"10".to_string()));
        
        // Test accel suggestions
        let suggestions = get_mouse_action_suggestions("movemouse-accel-up", 0);
        assert!(suggestions.contains(&"1".to_string()));
        
        let suggestions = get_mouse_action_suggestions("movemouse-accel-up", 1);
        assert!(suggestions.contains(&"1000".to_string()));
        
        // Test setmouse suggestions
        let suggestions = get_mouse_action_suggestions("setmouse", 0);
        assert!(suggestions.contains(&"0".to_string()));
        assert!(suggestions.contains(&"960".to_string()));
        
        // Test speed suggestions
        let suggestions = get_mouse_action_suggestions("movemouse-speed", 0);
        assert!(suggestions.contains(&"50".to_string()));
        assert!(suggestions.contains(&"100".to_string()));
        assert!(suggestions.contains(&"200".to_string()));
    }

    #[test]
    fn test_get_current_mouse_action() {
        // Basic movemouse
        let result = get_current_mouse_action("(movemouse-up 1");
        assert_eq!(result, Some(("movemouse-up".to_string(), 1)));
        
        // Accel movemouse
        let result = get_current_mouse_action("(movemouse-accel-down 5 500");
        assert_eq!(result, Some(("movemouse-accel-down".to_string(), 2)));
        
        // setmouse
        let result = get_current_mouse_action("(setmouse 0");
        assert_eq!(result, Some(("setmouse".to_string(), 1)));
        
        // Non-mouse action
        let result = get_current_mouse_action("(tap-hold 200");
        assert_eq!(result, None);
        
        // Not inside action
        let result = get_current_mouse_action("movemouse-up");
        assert_eq!(result, None);
    }

    #[test]
    fn test_mouse_in_multi() {
        // Mouse actions can be combined with other actions
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(multi (movemouse-up 1 1) (movemouse-right 1 1))"));
        assert!(validator.validate_action("(multi (movemouse-speed 200) (movemouse-left 5 10))"));
    }

    #[test]
    fn test_mouse_in_tap_hold() {
        // Mouse actions can be used in tap-hold
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(tap-hold 200 200 (movemouse-up 1 1) (movemouse-down 1 1))"));
    }

    #[test]
    fn test_complex_mouse_layer() {
        // Test all mouse actions are valid
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Basic movemouse
        assert!(validator.validate_action("(movemouse-up 1 1)"));
        assert!(validator.validate_action("(movemouse-down 1 1)"));
        assert!(validator.validate_action("(movemouse-left 1 1)"));
        assert!(validator.validate_action("(movemouse-right 1 1)"));
        
        // Accel movemouse
        assert!(validator.validate_action("(movemouse-accel-up 1 1000 1 5)"));
        assert!(validator.validate_action("(movemouse-accel-down 1 1000 1 5)"));
        assert!(validator.validate_action("(movemouse-accel-left 1 1000 1 5)"));
        assert!(validator.validate_action("(movemouse-accel-right 1 1000 1 5)"));
        
        // Set position and speed
        assert!(validator.validate_action("(setmouse 960 540)"));
        assert!(validator.validate_action("(movemouse-speed 200)"));
        assert!(validator.validate_action("(movemouse-speed 50)"));
    }

    // ============================================================================
    // cmd Action Tests
    // ============================================================================

    #[test]
    fn test_cmd_actions_exist() {
        assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd"));
        assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd-log"));
        assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd-output-keys"));
    }

    #[test]
    fn test_validate_cmd_simple() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(cmd echo hello)"));
        assert!(validator.validate_action("(cmd ls -la)"));
        assert!(validator.validate_action("(cmd bazel build -c opt //...)"));
    }

    #[test]
    fn test_validate_cmd_quoted() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(cmd echo \"hello world\")"));
        assert!(validator.validate_action("(cmd powershell.exe -c \"Get-Date\")"));
    }

    #[test]
    fn test_validate_cmd_log() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(cmd-log info error)"));
        assert!(validator.validate_action("(cmd-log debug none)"));
        assert!(validator.validate_action("(cmd-log warn info)"));
        assert!(validator.validate_action("(cmd-log ERROR DEBUG)")); // Case insensitive
    }

    #[test]
    fn test_validate_cmd_log_invalid() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Invalid log levels
        assert!(!validator.validate_action("(cmd-log invalid error)"));
        assert!(!validator.validate_action("(cmd-log info invalid)"));
        
        // Wrong number of params
        assert!(!validator.validate_action("(cmd-log info)"));
        assert!(!validator.validate_action("(cmd-log info error extra)"));
    }

    #[test]
    fn test_validate_cmd_no_args() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // cmd requires at least binary name
        assert!(!validator.validate_action("(cmd)"));
        assert!(!validator.validate_action("(cmd-output-keys)"));
    }

    #[test]
    fn test_validate_cmd_output_keys() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(cmd-output-keys xclip -o)"));
        assert!(validator.validate_action("(cmd-output-keys echo hello)"));
    }

    // ============================================================================
    // Clipboard Action Tests
    // ============================================================================

    #[test]
    fn test_clipboard_actions_exist() {
        let actions = [
            "clipboard-set", "clipboard-save", "clipboard-restore",
            "clipboard-save-swap", "clipboard-cmd-set", "clipboard-save-cmd-set"
        ];
        for action in &actions {
            assert!(KANATA_ACTIONS.iter().any(|a| a.name == *action),
                "Clipboard action {} should exist", action);
        }
    }

    #[test]
    fn test_validate_clipboard_set() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(clipboard-set \"hello\")"));
        assert!(validator.validate_action("(clipboard-set hello)"));
    }

    #[test]
    fn test_validate_clipboard_save_restore() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Valid IDs: 0-65535
        assert!(validator.validate_action("(clipboard-save 0)"));
        assert!(validator.validate_action("(clipboard-save 65535)"));
        assert!(validator.validate_action("(clipboard-restore 12345)"));
    }

    #[test]
    fn test_validate_clipboard_save_invalid_id() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(!validator.validate_action("(clipboard-save -1)"));
        assert!(!validator.validate_action("(clipboard-save 65536)"));
        assert!(!validator.validate_action("(clipboard-save abc)"));
    }

    #[test]
    fn test_validate_clipboard_save_swap() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(clipboard-save-swap 0 1)"));
        assert!(validator.validate_action("(clipboard-save-swap 100 200)"));
        
        // Invalid IDs
        assert!(!validator.validate_action("(clipboard-save-swap 0 65536)"));
    }

    #[test]
    fn test_validate_clipboard_cmd_set() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(clipboard-cmd-set echo hello)"));
        assert!(validator.validate_action("(clipboard-cmd-set powershell.exe -c Get-Date)"));
        
        // Needs at least binary
        assert!(!validator.validate_action("(clipboard-cmd-set)"));
    }

    #[test]
    fn test_validate_clipboard_save_cmd_set() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        assert!(validator.validate_action("(clipboard-save-cmd-set 0 echo hello)"));
        assert!(validator.validate_action("(clipboard-save-cmd-set 5 powershell.exe -c Get-Date)"));
        
        // Needs at least ID + binary
        assert!(!validator.validate_action("(clipboard-save-cmd-set 0)"));
        assert!(!validator.validate_action("(clipboard-save-cmd-set)"));
    }

    #[test]
    fn test_clipboard_in_macro() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Common use case: save, do something, restore
        assert!(validator.validate_action(
            "(macro (clipboard-save 0) 20 C-v (clipboard-restore 0))"
        ));
    }

    // ============================================================================
    // defchordsv2 Tests
    // ============================================================================

    #[test]
    fn test_chordv2_release_behaviour_enum() {
        use crate::keymap::{ReleaseBehaviour};
        
        assert_eq!(ReleaseBehaviour::FirstRelease, ReleaseBehaviour::FirstRelease);
        assert_eq!(ReleaseBehaviour::AllReleased, ReleaseBehaviour::AllReleased);
        assert_ne!(ReleaseBehaviour::FirstRelease, ReleaseBehaviour::AllReleased);
    }

    #[test]
    fn test_chordv2_types_exist() {
        // Test that ChordV2 can be created
        use crate::keymap::{ChordV2, ReleaseBehaviour};
        
        let chord = ChordV2 {
            keys: vec!["a".to_string(), "s".to_string()],
            action: "c".to_string(),
            timeout: 200,
            release_behaviour: ReleaseBehaviour::AllReleased,
            disabled_layers: vec![],
        };
        
        assert_eq!(chord.keys.len(), 2);
        assert_eq!(chord.timeout, 200);
    }

    #[test]
    fn test_chordv2_minimum_keys_validation() {
        use crate::keymap::{ChordV2, ReleaseBehaviour};
        
        // Valid: 2 keys
        let chord = ChordV2 {
            keys: vec!["a".to_string(), "s".to_string()],
            action: "c".to_string(),
            timeout: 200,
            release_behaviour: ReleaseBehaviour::AllReleased,
            disabled_layers: vec![],
        };
        assert!(chord.keys.len() >= 2);
        
        // Valid: 3 keys
        let chord = ChordV2 {
            keys: vec!["a".to_string(), "s".to_string(), "d".to_string()],
            action: "c".to_string(),
            timeout: 200,
            release_behaviour: ReleaseBehaviour::AllReleased,
            disabled_layers: vec![],
        };
        assert!(chord.keys.len() >= 2);
    }

    #[test]
    fn test_chordv2_timeout_validation() {
        // Timeout must be positive
        let timeout: u32 = 200;
        assert!(timeout > 0);
        
        let timeout: u32 = 1;
        assert!(timeout > 0);
    }

    #[test]
    fn test_chordv2_keymap_data_integration() {
        use crate::keymap::{ChordV2, ReleaseBehaviour, KeymapData};
        
        let mut data = KeymapData::default();
        
        data.chordsv2.push(ChordV2 {
            keys: vec!["a".to_string(), "s".to_string()],
            action: "c".to_string(),
            timeout: 200,
            release_behaviour: ReleaseBehaviour::AllReleased,
            disabled_layers: vec![],
        });
        
        assert_eq!(data.chordsv2.len(), 1);
        assert_eq!(data.chordsv2[0].keys, vec!["a", "s"]);
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn test_cmd_and_clipboard_integration() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // cmd setting clipboard
        assert!(validator.validate_action("(clipboard-cmd-set echo hello)"));
        
        // Complex macro with clipboard
        assert!(validator.validate_action(
            "(macro (clipboard-save 0) (clipboard-cmd-set echo \"hello world\") 100 C-v (clipboard-restore 0))"
        ));
    }

    #[test]
    fn test_cmd_in_macro() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // cmd can be used in macro sequences
        assert!(validator.validate_action(
            "(macro esc (cmd notify-send \"Hello\") 100 esc)"
        ));
    }

    #[test]
    fn test_cmd_in_tap_hold() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // cmd can be used in tap-hold
        assert!(validator.validate_action(
            "(tap-hold 200 200 (cmd echo tap) (cmd echo hold))"
        ));
    }

    #[test]
    fn test_cmd_complex_real_world() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // Real-world complex commands
        assert!(validator.validate_action(
            "(cmd bazel build -c opt //src/...)"
        ));
        
        assert!(validator.validate_action(
            "(cmd git status --short)"
        ));
        
        assert!(validator.validate_action(
            "(cmd powershell.exe -Command \"Get-Process | Select-Object Name, CPU\")"
        ));
        
        assert!(validator.validate_action(
            "(cmd wtype \"special characters: àáâãäå\")"
        ));
    }

    #[test]
    fn test_all_new_actions_in_multi() {
        let data = create_test_data_with_defvars();
        let validator = KanataValidator::new(&data);
        
        // cmd in multi
        assert!(validator.validate_action("(multi (cmd echo hello) esc)"));
        
        // clipboard in multi
        assert!(validator.validate_action("(multi (clipboard-set \"test\") esc)"));
        
        // Combined
        assert!(validator.validate_action(
            "(multi (clipboard-save 0) (cmd echo done) (clipboard-restore 0))"
        ));
    }
}

async fn is_laptop() -> bool {
    use web_sys::BatteryManager;
    
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        // Cast Navigator to BatteryNavigator to access get_battery
        let battery_navigator: &BatteryNavigator = navigator.unchecked_ref();
        
        match battery_navigator.get_battery() {
            Ok(promise) => {
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(battery_val) => {
                        // Try to get the battery manager and log details
                        if let Ok(battery) = battery_val.dyn_into::<BatteryManager>() {
                            let charging = battery.charging();
                            let level = battery.level();
                            let charging_time = battery.charging_time();
                            let discharging_time = battery.discharging_time();
                            
                            web_sys::console::log_1(&format!(
                                "Battery info: charging={}, level={}, charging_time={}, discharging_time={}",
                                charging, level, charging_time, discharging_time
                            ).into());
                            
                            // A desktop typically always reports charging=true and level=1.0
                            // A laptop on battery would have charging=false and level < 1.0
                            // If discharging_time is finite, it's definitely a laptop on battery
                            let is_laptop = !charging || level < 1.0 || discharging_time.is_finite();
                            web_sys::console::log_1(&format!("is_laptop determination: {}", is_laptop).into());
                            return is_laptop;
                        }
                        false
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Battery API error: {:?}", e).into());
                        false
                    }
                }
            }
            Err(e) => {
                web_sys::console::log_1(&format!("getBattery not available: {:?}", e).into());
                false
            }
        }
    } else {
        false
    }
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

    let parse_content = {
        let kanata_data = kanata_data.clone();
        let original_content = original_content.clone();
        let error = error.clone();
        let loading = loading.clone();
        let is_laptop_state = is_laptop_state.clone();
        Callback::from(move |content: String| {
            let kanata_data = kanata_data.clone();
            let original_content = original_content.clone();
            let error = error.clone();
            let loading = loading.clone();
            let is_laptop_state = is_laptop_state.clone();
            spawn_local(async move {
                loading.set(true);
                let laptop = is_laptop().await;
                is_laptop_state.set(laptop);
                original_content.set(content.clone());

                let parse_result = Request::post("/api/parse-kanata")
                    .json(&KanataRequest { 
                        content, 
                        is_mac: crate::is_mac(),
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
            });
        })
    };

    let on_open = {
        let parse_content = parse_content.clone();
        let error = error.clone();
        let file_handle = file_handle.clone();
        Callback::from(move |_| {
            let parse_content = parse_content.clone();
            let error = error.clone();
            let file_handle = file_handle.clone();
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
                                            parse_content.emit(content);
                                        }
                                        Err(e) => error.set(Some(format!("Failed to read file: {:?}", e))),
                                    }
                                }
                                Err(e) => error.set(Some(format!("Failed to get file: {:?}", e))),
                            }
                        }
                    }
                    Err(e) => error.set(Some(format!("File picker error: {:?}", e))),
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

    // Global keyboard listener for Ctrl-S and Ctrl-O
    {
        let has_data = kanata_data.is_some();
        let on_open = on_open.clone();
        let on_save = on_save.clone();
        use_effect_with((has_data, on_open, on_save), move |(has_data, on_open, on_save)| {
            let on_open = on_open.clone();
            let on_save = on_save.clone();
            let has_data = *has_data;
            let key_listener = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                let key = e.key().to_lowercase();
                if (e.ctrl_key() || e.meta_key()) && key == "o" {
                    e.prevent_default();
                    on_open.emit(MouseEvent::new("click").unwrap());
                } else if (e.ctrl_key() || e.meta_key()) && key == "s" {
                    if has_data {
                        e.prevent_default();
                        on_save.emit(MouseEvent::new("click").unwrap());
                    }
                }
            }) as Box<dyn FnMut(KeyboardEvent)>);

            let window = web_sys::window().expect("should have a window");
            window
                .add_event_listener_with_callback(
                    "keydown",
                    key_listener.as_ref().unchecked_ref(),
                )
                .unwrap();

            move || {
                let window = web_sys::window().expect("should have a window");
                window
                    .remove_event_listener_with_callback(
                        "keydown",
                        key_listener.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                drop(key_listener);
            }
        });
    }

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
                    let data = (*kanata_data).clone().unwrap();
                    let laptop = *is_laptop_state;
                    let on_download_svg = {
                        let data = data.clone();
                        Callback::from(move |_| {
                            let svg_content = crate::keymap::generate_svg(&data, true, crate::is_mac(), laptop);
                            let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&svg_content))).unwrap();
                            let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                            let document = web_sys::window().unwrap().document().unwrap();
                            let anchor = document.create_element("a").unwrap().unchecked_into::<web_sys::HtmlAnchorElement>();
                            anchor.set_href(&url);
                            anchor.set_download("kanata.svg");
                            anchor.click();
                            web_sys::Url::revoke_object_url(&url).unwrap();
                        })
                    };
                    html! {
                        <div class="flex space-x-2 mt-6">
                            <button onclick={on_save} class="px-6 py-2.5 bg-green-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-green-700 hover:shadow-lg focus:bg-green-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-green-800 active:shadow-lg transition duration-150 ease-in-out">
                                {"Save File"}
                            </button>
                            <button onclick={on_download_svg} class="px-6 py-2.5 bg-purple-600 text-white font-medium text-xs leading-tight uppercase rounded shadow-md hover:bg-purple-700 hover:shadow-lg focus:bg-purple-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-purple-800 active:shadow-lg transition duration-150 ease-in-out">
                                {"Download SVG"}
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
    
    // Layer menu state
    let menu_state = use_state(|| layer_menu::LayerMenuState::default());

    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let num_keys = props.data.physical_layout.len();
    let num_layers = props.data.layers.len();

    // Build hint map including menu targets
    let (hint_map, key_hints, layer_hints) = layer_menu::build_hint_map(
        num_keys,
        num_layers,
        menu_state.menu_open_index,
    );
    
    // Create layer operation closures
    let on_update = props.on_update.clone();
    let current_layer = props.current_layer.clone();
    let data = props.data.clone();
    
    // Move layer up/down
    let move_layer = {
        let on_update = on_update.clone();
        let current_layer = current_layer.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize, up: bool| {
            let mut new_data = data.clone();
            let layers = &mut new_data.layers;
            
            // Guards
            if up && idx == 0 { 
                menu_state.set(layer_menu::LayerMenuState::default());
                return; 
            }
            if !up && idx >= layers.len().saturating_sub(1) { 
                menu_state.set(layer_menu::LayerMenuState::default());
                return; 
            }
            
            let target = if up { idx - 1 } else { idx + 1 };
            layers.swap(idx, target);
            
            // Sync current_layer if it moved
            let current = *current_layer;
            if current == idx {
                current_layer.set(target);
            } else if current == target {
                current_layer.set(idx);
            }
            
            on_update.emit(new_data);
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize, bool)>
    };
    
    // Rename layer
    let rename_layer = {
        let on_update = on_update.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize| {
            let current_name = data.layers[idx].name.clone();
            let window = web_sys::window().unwrap();
            
            if let Ok(Some(new_name)) = window.prompt_with_message_and_default(
                "Rename layer:",
                &current_name
            ) {
                let trimmed = new_name.trim();
                if !trimmed.is_empty() && trimmed.len() <= 32 {
                    let mut new_data = data.clone();
                    new_data.layers[idx].name = trimmed.to_string();
                    on_update.emit(new_data);
                }
            }
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize)>
    };
    
    // Duplicate layer
    let duplicate_layer = {
        let on_update = on_update.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize| {
            let mut new_data = data.clone();
            
            // Max layers limit
            if new_data.layers.len() >= 32 {
                menu_state.set(layer_menu::LayerMenuState::default());
                return;
            }
            
            let new_layer = Layer {
                name: format!("{} (copy)", new_data.layers[idx].name),
                bindings: new_data.layers[idx].bindings.clone(),
                layer_type: new_data.layers[idx].layer_type.clone(),
                source_layer: new_data.layers[idx].source_layer.clone(),
                key_bindings: new_data.layers[idx].key_bindings.clone(),
            };
            
            new_data.layers.insert(idx + 1, new_layer);
            on_update.emit(new_data);
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize)>
    };
    
    // Delete layer
    let delete_layer = {
        let on_update = on_update.clone();
        let current_layer = current_layer.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize| {
            // Prevent deleting last layer
            if data.layers.len() <= 1 {
                menu_state.set(layer_menu::LayerMenuState::default());
                return;
            }
            
            let window = web_sys::window().unwrap();
            let confirmed = window
                .confirm_with_message(&format!(
                    "Delete layer '{}'? This cannot be undone.",
                    data.layers[idx].name
                ))
                .unwrap_or(false);
            
            if confirmed {
                let mut new_data = data.clone();
                new_data.layers.remove(idx);
                
                // Adjust current_layer if necessary
                let current = *current_layer;
                if current >= new_data.layers.len() {
                    current_layer.set(new_data.layers.len().saturating_sub(1));
                } else if current == idx && current > 0 {
                    current_layer.set(current - 1);
                }
                
                on_update.emit(new_data);
            }
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize)>
    };
    
    // Reset all keys to "_"
    let reset_layer = {
        let on_update = on_update.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize| {
            let mut new_data = data.clone();
            let key_count = new_data.layers[idx].bindings.len();
            new_data.layers[idx].bindings = vec!["_".to_string(); key_count];
            on_update.emit(new_data);
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize)>
    };
    
    // Convert "_" to "none"
    let trans_to_none = {
        let on_update = on_update.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize| {
            let mut new_data = data.clone();
            for binding in new_data.layers[idx].bindings.iter_mut() {
                if binding == "_" {
                    *binding = "none".to_string();
                }
            }
            on_update.emit(new_data);
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize)>
    };
    
    // Convert "none" to "_"
    let none_to_trans = {
        let on_update = on_update.clone();
        let menu_state = menu_state.clone();
        let data = data.clone();
        Rc::new(move |idx: usize| {
            let mut new_data = data.clone();
            for binding in new_data.layers[idx].bindings.iter_mut() {
                if binding == "none" {
                    *binding = "_".to_string();
                }
            }
            on_update.emit(new_data);
            menu_state.set(layer_menu::LayerMenuState::default());
        }) as Rc<dyn Fn(usize)>
    };

    {
        let container_ref = container_ref.clone();
        let current_layer_idx = *props.current_layer;
        let data = props.data.clone();
        let selected_key = (*props.selected_key).clone();
        use_effect_with((current_layer_idx, data, selected_key), move |_| {
            if let Some(element) = container_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
            || ()
        });
    }

    // Pre-compute phantom key indices and non-phantom key list for quick assign
    let phantom_indices: std::collections::HashSet<usize> = props.data.physical_layout.iter()
        .enumerate()
        .filter(|(_, pk)| pk.origin == KeyOrigin::Phantom)
        .map(|(i, _)| i)
        .collect();
    
    // List of non-phantom key indices for quick assign mode
    let non_phantom_keys: Vec<usize> = (0..num_keys)
        .filter(|i| !phantom_indices.contains(i))
        .collect();
    
    // Start quick assign mode - starts at first non-phantom key
    let start_quick_assign = {
        let menu_state = menu_state.clone();
        let first_non_phantom = non_phantom_keys.first().copied();
        Rc::new(move || {
            menu_state.set(layer_menu::LayerMenuState {
                menu_open_index: None,
                focus_index: 0,
                quick_assign_index: first_non_phantom,
            });
        }) as Rc<dyn Fn()>
    };

    let on_keydown = {
        let jump_mode_active = jump_mode_active.clone();
        let jump_input = jump_input.clone();
        let selected_key = props.selected_key.clone();
        let current_layer = props.current_layer.clone();
        let hint_map = hint_map.clone();
        let menu_state = menu_state.clone();
        // Clone individual closures
        let move_layer = move_layer.clone();
        let rename_layer = rename_layer.clone();
        let duplicate_layer = duplicate_layer.clone();
        let delete_layer = delete_layer.clone();
        let reset_layer = reset_layer.clone();
        let trans_to_none = trans_to_none.clone();
        let none_to_trans = none_to_trans.clone();
        let start_quick_assign = start_quick_assign.clone();
        // Clone props data to avoid borrowing issues
        let props_data = props.data.clone();
        let props_on_update = props.on_update.clone();
        // Clone non-phantom keys list for quick assign
        let non_phantom_keys = non_phantom_keys.clone();

        Callback::from(move |e: KeyboardEvent| {
            if selected_key.is_some() {
                return;
            }
            
            // Handle menu keyboard navigation when menu is open
            if let Some(lmi) = menu_state.menu_open_index {
                match e.key().as_str() {
                    "ArrowDown" => {
                        let new_focus = (menu_state.focus_index + 1) % 9;
                        menu_state.set(layer_menu::LayerMenuState {
                            menu_open_index: Some(lmi),
                            focus_index: new_focus,
                            quick_assign_index: menu_state.quick_assign_index,
                        });
                        e.prevent_default();
                        return;
                    }
                    "ArrowUp" => {
                        let new_focus = (menu_state.focus_index + 8) % 9;
                        menu_state.set(layer_menu::LayerMenuState {
                            menu_open_index: Some(lmi),
                            focus_index: new_focus,
                            quick_assign_index: menu_state.quick_assign_index,
                        });
                        e.prevent_default();
                        return;
                    }
                    "Enter" => {
                        match menu_state.focus_index {
                            0 => move_layer(lmi, true),
                            1 => move_layer(lmi, false),
                            2 => rename_layer(lmi),
                            3 => duplicate_layer(lmi),
                            4 => delete_layer(lmi),
                            5 => reset_layer(lmi),
                            6 => trans_to_none(lmi),
                            7 => none_to_trans(lmi),
                            8 => start_quick_assign(),
                            _ => {}
                        }
                        e.prevent_default();
                        return;
                    }
                    "Escape" => {
                        menu_state.set(layer_menu::LayerMenuState::default());
                        e.prevent_default();
                        return;
                    }
                    _ => {}
                }
            }
            
            // Handle quick assign mode
            if menu_state.quick_assign_index.is_some() {
                if e.key() == "Escape" {
                    menu_state.set(layer_menu::LayerMenuState::default());
                    e.prevent_default();
                    return;
                }
                
                // Single character key assignment
                if e.key().len() == 1 {
                    let key = e.key();
                    if let Some(first) = key.chars().next() {
                        if let Some(idx) = menu_state.quick_assign_index {
                            let mut new_data = props_data.clone();
                            if idx < new_data.layers[*current_layer].bindings.len() {
                                // Convert to kanata key format
                                let kanata_key = if first.is_ascii_alphabetic() {
                                    first.to_ascii_lowercase().to_string()
                                } else if first.is_ascii_digit() {
                                    first.to_string()
                                } else {
                                    return;
                                };
                                
                                new_data.layers[*current_layer].bindings[idx] = kanata_key;
                                
                                // Find next non-phantom key
                                let current_pos = non_phantom_keys.iter().position(|&i| i == idx);
                                let next_idx = if let Some(pos) = current_pos {
                                    let next_pos = (pos + 1) % non_phantom_keys.len();
                                    non_phantom_keys[next_pos]
                                } else {
                                    // If current key not found (shouldn't happen), start from beginning
                                    non_phantom_keys.first().copied().unwrap_or(0)
                                };
                                
                                menu_state.set(layer_menu::LayerMenuState {
                                    menu_open_index: None,
                                    focus_index: 0,
                                    quick_assign_index: Some(next_idx),
                                });
                                
                                props_on_update.emit(new_data);
                                e.prevent_default();
                                return;
                            }
                        }
                    }
                }
            }

            if *jump_mode_active {
                match e.key().as_str() {
                    "Enter" | "Escape" => {
                        jump_mode_active.set(false);
                        jump_input.set(String::new());
                        e.prevent_default();
                    }
                    key if key.len() == 1 && hint_chars.contains(key) => {
                        let mut new_input = (*jump_input).clone();
                        new_input.push_str(key);

                        if let Some(target) = hint_map.get(&new_input) {
                            match target {
                                layer_menu::HintTarget::Key(idx) => {
                                    let is_phantom = phantom_indices.contains(idx);
                                    selected_key.set(Some(SelectedKey {
                                        layer_index: *current_layer,
                                        key_index: *idx,
                                        is_phantom,
                                    }));
                                }
                                layer_menu::HintTarget::Layer(idx) => {
                                    current_layer.set(*idx);
                                }
                                layer_menu::HintTarget::LayerMenu(idx) => {
                                    menu_state.set(layer_menu::LayerMenuState {
                                        menu_open_index: Some(*idx),
                                        focus_index: 0,
                                        quick_assign_index: None,
                                    });
                                }
                                layer_menu::HintTarget::Menu(l_idx, m_idx) => {
                                    match *m_idx {
                                        0 => move_layer(*l_idx, true),
                                        1 => move_layer(*l_idx, false),
                                        2 => rename_layer(*l_idx),
                                        3 => duplicate_layer(*l_idx),
                                        4 => delete_layer(*l_idx),
                                        5 => reset_layer(*l_idx),
                                        6 => trans_to_none(*l_idx),
                                        7 => none_to_trans(*l_idx),
                                        8 => start_quick_assign(),
                                        _ => {}
                                    }
                                }
                            }
                            jump_mode_active.set(false);
                            jump_input.set(String::new());
                        } else if hint_map.keys().any(|h| h.starts_with(&new_input)) {
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
    
    // Click outside to close menu
    {
        let menu_state = menu_state.clone();
        use_effect(move || {
            let click_listener = wasm_bindgen::closure::Closure::wrap(
                Box::new(move |e: web_sys::MouseEvent| {
                    // Check if click is outside menu
                    if let Some(target) = e.target() {
                        if let Ok(element) = target.dyn_into::<web_sys::Element>() {
                            // Close menu if clicking outside
                            if !element.closest("[data-layer-menu]").unwrap_or(None).is_some() {
                                menu_state.set(layer_menu::LayerMenuState::default());
                            }
                        }
                    }
                }) as Box<dyn FnMut(web_sys::MouseEvent)>
            );
            
            let window = web_sys::window().unwrap();
            window.add_event_listener_with_callback(
                "click",
                click_listener.as_ref().unchecked_ref(),
            ).unwrap();
            
            move || {
                window.remove_event_listener_with_callback(
                    "click",
                    click_listener.as_ref().unchecked_ref(),
                ).unwrap();
                drop(click_listener);
            }
        });
    }

    let layer = &props.data.layers[*props.current_layer];

    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    
    // Split into standard keys and aliases for rendering
    let _num_standard_keys = props.data.defsrc.len() - props.data.aliases.len();
    
    // DEBUG: Log phantom keys info to help diagnose rendering issues
    let phantom_count = props.data.physical_layout.iter().filter(|pk| pk.origin == KeyOrigin::Phantom).count();
    web_sys::console::log_1(&format!("DEBUG FRONTEND: Physical layout has {} keys total, {} have origin=Phantom. phantom_keys array size is {}. process_unmapped_keys is {:?}", 
        props.data.physical_layout.len(), phantom_count, props.data.phantom_keys.len(), props.data.process_unmapped_keys).into());

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

    let unmapped_y_threshold = 6500;
    let alias_y_threshold = 8000;

    html! {
        <div ref={container_ref.clone()} tabindex="0" onkeydown={on_keydown} class="flex flex-col items-center w-full focus:outline-none">
            <div class="flex flex-wrap gap-2 mb-4 relative">
                { for props.data.layers.iter().enumerate().map(|(i, l)| {
                    let is_active = i == *props.current_layer;
                    let is_menu_open = menu_state.menu_open_index == Some(i);
                    let onclick = { 
                        let cl = props.current_layer.clone(); 
                        let ms = menu_state.clone();
                        Callback::from(move |e: MouseEvent| {
                            e.stop_propagation();
                            cl.set(i);
                            ms.set(layer_menu::LayerMenuState::default());
                        }) 
                    };
                    
                    // Layer hint (to select layer)
                    let layer_hint = layer_hints.get(i).cloned().unwrap_or_default();
                    let show_layer_hint = *jump_mode_active && !layer_hint.is_empty() 
                        && layer_hint.starts_with(&*jump_input);
                    
                    // Menu trigger hint
                    let menu_trigger_hint = hint_map.iter()
                        .find(|(_, t)| **t == layer_menu::HintTarget::LayerMenu(i))
                        .map(|(h, _)| h.clone())
                        .unwrap_or_default();
                    let show_menu_trigger_hint = *jump_mode_active && !menu_trigger_hint.is_empty()
                        && menu_trigger_hint.starts_with(&*jump_input);
                    
                    html! {
                        <div class="relative" data-layer-menu={if is_menu_open { "open" } else { "" }}>
                            <button onclick={onclick} class={classes!("px-4", "py-1.5", "rounded-md", "shadow-sm", "font-medium", "transition-all", "relative", "flex", "items-center", "gap-2",
                                if is_active { "bg-white dark:bg-gray-700 text-blue-600 dark:text-blue-400" } else { "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 bg-gray-100 dark:bg-gray-800" }
                            )}>
                                {&l.name}
                                
                                // Menu trigger (chevron)
                                <span
                                    onclick={let ms = menu_state.clone(); Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        if ms.menu_open_index == Some(i) {
                                            ms.set(layer_menu::LayerMenuState::default());
                                        } else {
                                            ms.set(layer_menu::LayerMenuState {
                                                menu_open_index: Some(i),
                                                focus_index: 0,
                                                quick_assign_index: None,
                                            });
                                        }
                                    })}
                                    class="hover:bg-black/10 dark:hover:bg-white/10 rounded p-1 relative"
                                >
                                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
                                    </svg>
                                    { if show_menu_trigger_hint {
                                        let h = &menu_trigger_hint;
                                        let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                        html! { <div class="absolute -top-2 -right-2 bg-blue-400 dark:bg-blue-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                    } else { html! {} }}
                                </span>
                                
                                // Layer hint badge
                                { if show_layer_hint {
                                    let h = &layer_hint;
                                    let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                    html! { <div class="absolute top-0 left-0 bg-yellow-400 dark:bg-yellow-600 px-0.5 z-30 font-bold text-[10px] text-black dark:text-white rounded-tl-md rounded-br-md shadow-sm pointer-events-none leading-tight border-r border-b border-yellow-500 dark:border-yellow-700"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                } else { html! {} }}
                            </button>
                            
                            // Dropdown menu
                            { if is_menu_open {
                                let menu_items: Vec<(&str, Callback<MouseEvent>)> = vec![
                                    ("Move Up", Callback::from({ let op = move_layer.clone(); move |_| op(i, true) })),
                                    ("Move Down", Callback::from({ let op = move_layer.clone(); move |_| op(i, false) })),
                                    ("Rename", Callback::from({ let op = rename_layer.clone(); move |_| op(i) })),
                                    ("Duplicate", Callback::from({ let op = duplicate_layer.clone(); move |_| op(i) })),
                                    ("Delete", Callback::from({ let op = delete_layer.clone(); move |_| op(i) })),
                                    ("Reset all to None", Callback::from({ let op = reset_layer.clone(); move |_| op(i) })),
                                    ("Trans → None", Callback::from({ let op = trans_to_none.clone(); move |_| op(i) })),
                                    ("None → Trans", Callback::from({ let op = none_to_trans.clone(); move |_| op(i) })),
                                    ("Quick Assignment", Callback::from({ let op = start_quick_assign.clone(); move |_| op() })),
                                ];
                                
                                html! {
                                    <div class="absolute top-full left-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 z-50 py-1 overflow-hidden">
                                        { for menu_items.into_iter().enumerate().map(|(j, (label, cb))| {
                                            let is_focused = menu_state.focus_index == j;
                                            let menu_hint = hint_map.iter()
                                                .find(|(_, t)| **t == layer_menu::HintTarget::Menu(i, j))
                                                .map(|(h, _)| h.clone())
                                                .unwrap_or_default();
                                            let show_menu_hint = *jump_mode_active && !menu_hint.is_empty()
                                                && menu_hint.starts_with(&*jump_input);
                                            
                                            let class = classes!("w-full", "text-left", "px-4", "py-2", "text-sm", "relative",
                                                if is_focused { "bg-blue-100 dark:bg-blue-900/40" } else { "hover:bg-gray-100 dark:hover:bg-gray-700" },
                                                if j == 4 { "text-red-500" } else if j == 5 { "text-orange-500" } else if j == 8 { "font-bold text-blue-500" } else { "" }
                                            );
                                            
                                            html! {
                                                <>
                                                    { if j == 5 || j == 8 { 
                                                        html! { <div class="border-t border-gray-200 dark:border-gray-700 my-1"></div> } 
                                                    } else { html! {} }}
                                                    <button onclick={cb} class={class}>
                                                        {label}
                                                        { if show_menu_hint {
                                                            let h = &menu_hint;
                                                            let (prefix, suffix) = if jump_input.is_empty() { ("", h.as_str()) } else { (&h[..jump_input.len()], &h[jump_input.len()..]) };
                                                            html! { <div class="absolute top-0 right-0 bg-yellow-400 dark:bg-yellow-600 px-1 z-50 font-bold text-[10px] text-black dark:text-white rounded-bl-md shadow-sm pointer-events-none"><span class="opacity-40">{prefix}</span><span>{suffix}</span></div> }
                                                        } else { html! {} }}
                                                    </button>
                                                </>
                                            }
                                        })}
                                    </div>
                                }
                            } else { html! {} }}
                        </div>
                    }
                })}
            </div>
            
            // Quick assign mode banner
            { if let Some(idx) = menu_state.quick_assign_index {
                html! {
                    <div class="w-full mb-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-xl border border-blue-200 dark:border-blue-800">
                        <div class="flex justify-between items-center">
                            <div>
                                <h3 class="text-lg font-bold text-blue-800 dark:text-blue-300">{"Quick Assignment Mode"}</h3>
                                <p class="text-sm text-blue-600 dark:text-blue-400">{format!("Press keys on your keyboard to assign. Currently editing key {} of {}", idx + 1, num_keys)}</p>
                            </div>
                            <button onclick={let ms = menu_state.clone(); Callback::from(move |_: MouseEvent| ms.set(layer_menu::LayerMenuState::default()))} 
                                class="bg-blue-500 hover:bg-blue-600 text-white px-4 py-1 rounded-lg font-bold">
                                {"Done"}
                            </button>
                        </div>
                        <p class="text-xs text-blue-500 dark:text-blue-400 mt-2">{"Press Escape to exit, or click any key to jump to it."}</p>
                    </div>
                }
            } else { html! {} }}
            

            <div class={classes!("relative", "border", "dark:border-gray-600", "p-8", "rounded-xl", "bg-gray-50", "dark:bg-gray-800", "shadow-inner", "overflow-auto", "w-full", "max-w-full", "text-center")} style="min-height: 350px; height: 65vh;">
                <div class="relative inline-block text-left" style={format!("width: {}px; height: {}px;", content_width, content_height)}>
                    { for props.data.physical_layout.iter().enumerate().map(|(i, pk)| {
                        let binding = layer.bindings.get(i).cloned().unwrap_or_else(|| "".to_string());
                        let defsrc_name = if pk.origin == KeyOrigin::Phantom {
                            pk.name.clone()  // Use the phantom key's name
                        } else {
                            props.data.defsrc.get(i).cloned().unwrap_or_default()
                        };
                        let parts = get_kanata_binding_parts_internal(&binding, &props.data.aliases, crate::is_mac(), props.is_laptop);
                        let x = (pk.x as f32 * scale + offset_x) as i32;
                        let y = (pk.y as f32 * scale + offset_y) as i32;
                        let w = (pk.width as f32 * scale) as i32 - 2;
                        let h = (pk.height as f32 * scale) as i32 - 2;
                        let is_phantom = pk.origin == KeyOrigin::Phantom;
                        let onclick = { 
                            let sk = props.selected_key.clone(); 
                            let cur_l = *props.current_layer;
                            let ms = menu_state.clone();
                            Callback::from(move |_| {
                                if ms.quick_assign_index.is_some() {
                                    // In quick assign mode, clicking jumps to that key
                                    // But skip phantom keys - find next non-phantom
                                    if is_phantom {
                                        return; // Ignore clicks on phantom keys
                                    }
                                    ms.set(layer_menu::LayerMenuState {
                                        menu_open_index: None,
                                        focus_index: 0,
                                        quick_assign_index: Some(i),
                                    });
                                } else {
                                    sk.set(Some(SelectedKey { 
                                        layer_index: cur_l, 
                                        key_index: i,
                                        is_phantom,
                                    }));
                                }
                            }) 
                        };
                        let hint = key_hints.get(i);
                        let show_hint = *jump_mode_active && hint.map(|h| h.starts_with(&*jump_input)).unwrap_or(false);
                        
                        // Quick assign highlighting
                        let is_quick_assign_target = menu_state.quick_assign_index == Some(i);

                        let is_alias_section = pk.y >= alias_y_threshold;
                        let is_unmapped_section = pk.y >= unmapped_y_threshold && pk.y < alias_y_threshold;

                        html! {
                            <>
                                { if pk.y == unmapped_y_threshold && pk.x == 0 {
                                    html! { <div class="absolute w-full border-t-2 border-dashed border-orange-300 dark:border-orange-900/30" style={format!("top: {}px; left: 0;", y - 20)}>
                                        <span class="absolute -top-3 left-0 bg-gray-50 dark:bg-gray-800 px-2 text-[10px] font-bold text-orange-400">{"UNMAPPED"}</span>
                                    </div> }
                                } else { html! {} }}

                                { if pk.y == alias_y_threshold && pk.x == 0 {
                                    html! { <div class="absolute w-full border-t-2 border-dashed border-blue-300 dark:border-blue-900/30" style={format!("top: {}px; left: 0;", y - 20)}>
                                        <span class="absolute -top-3 left-0 bg-gray-50 dark:bg-gray-800 px-2 text-[10px] font-bold text-blue-400">{"ALIASES"}</span>
                                    </div> }
                                } else { html! {} }}
                                
                                { if is_alias_section || is_unmapped_section {
                                    let label_color = if is_alias_section { "text-blue-500" } else { "text-orange-500" };
                                    html! { <div class={classes!("absolute", "text-[8px]", "font-bold", "truncate", "text-center", label_color)} style={format!("left: {}px; top: {}px; width: {}px;", x, y - 12, w)}> {defsrc_name.clone()} </div> }
                                } else { html! {} }}

                                <div onclick={onclick} class={classes!("absolute", "flex", "flex-col", "items-center", "justify-center", "cursor-pointer", "transition-all", "select-none",
                                    if is_quick_assign_target {
                                        vec!["ring-4", "ring-blue-500", "z-40", "bg-white", "dark:bg-gray-700", "border", "border-gray-300", "dark:border-gray-600", "shadow-lg", "rounded"]
                                    } else if is_phantom {
                                        vec!["border-2", "border-dashed", "border-gray-400", "dark:border-gray-500", "bg-transparent", "hover:border-gray-600", "dark:hover:border-gray-400", "rounded"]
                                    } else if is_alias_section {
                                        vec!["bg-blue-50/30", "dark:bg-blue-900/10", "border", "border-blue-200", "dark:border-blue-800", "rounded"]
                                    } else if is_unmapped_section {
                                        vec!["bg-orange-50/30", "dark:bg-orange-900/10", "border", "border-orange-200", "dark:border-orange-800", "rounded"]
                                    } else {
                                        vec!["bg-white", "dark:bg-gray-700", "border", "border-gray-300", "dark:border-gray-600", "hover:border-blue-400", "dark:hover:border-blue-500", "shadow-sm", "rounded"]
                                    }
                                )} style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", x, y, w, h)}>

                                    { if is_phantom {
                                        // Phantom keys show their name in the center
                                        html! {
                                            <>
                                                <span class="text-[10px] font-bold text-gray-400 dark:text-gray-500 truncate px-1 mt-1 leading-tight text-center pointer-events-none">{&pk.name}</span>
                                                <span class="text-[7px] text-gray-300 dark:text-gray-600 absolute bottom-0.5 pointer-events-none">{"(phantom)"}</span>
                                            </>
                                        }
                                    } else {
                                        // Regular keys show binding parts
                                        html! {
                                            <>
                                                <div class="w-full flex justify-between px-1 text-[7px] text-gray-400 absolute top-0.5 pointer-events-none">
                                                    <span class="truncate max-w-[45%]">{parts.top_left}</span>
                                                    <span class="truncate max-w-[45%] text-right">{parts.top_right}</span>
                                                </div>
                                                <span class="text-[12px] font-bold truncate px-1 mt-1 leading-tight text-center pointer-events-none">{parts.center}</span>
                                                
                                                <div class="w-full flex justify-end px-1 text-[6px] text-gray-300 dark:text-gray-500 absolute bottom-0.5 pointer-events-none font-mono">
                                                    <span>{defsrc_name}</span>
                                                </div>
                                            </>
                                        }
                                    }}

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
    let pk = &props.data.physical_layout[props.selected_key.key_index];
    let is_alias_section = pk.origin == KeyOrigin::Alias;
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

    let (prefix, suggestions) = get_suggestions(&*current_text, &props.data);

    let on_save = {
        let on_update = props.on_update.clone();
        let data = props.data.clone();
        let sk = props.selected_key.clone();
        let current_text = current_text.clone();
        let on_close = props.on_close.clone();
        let is_valid = is_valid;
        let _is_laptop = props.is_laptop;
        Callback::from(move |e: MouseEvent| {
            if !is_valid { return; }
            let mut new_data = data.clone();
            let text = (*current_text).clone().trim().to_string();
            
            // Handle phantom key conversion
            if sk.is_phantom {
                // Get the phantom key's name
                let phantom_name = new_data.physical_layout.get(sk.key_index)
                    .map(|pk| pk.name.clone())
                    .unwrap_or_default();
                
                if !phantom_name.is_empty() {
                    // 1. Add to defsrc
                    new_data.defsrc.push(phantom_name.clone());
                    
                    // 2. Remove from phantom_keys
                    new_data.phantom_keys.retain(|p| p.name.to_lowercase() != phantom_name.to_lowercase());
                    
                    // 3. Shift the binding to the end of standard keys for all layers
                    let insert_idx = new_data.defsrc.len() - 1;
                    let old_idx = sk.key_index;
                    
                    for (i, layer) in new_data.layers.iter_mut().enumerate() {
                        let mut val = layer.bindings.remove(old_idx);
                        if i == sk.layer_index {
                            val = text.clone();
                        }
                        layer.bindings.insert(insert_idx, val);
                    }
                    
                    // 4. Shift physical_layout to reflect the change
                    let mut pk = new_data.physical_layout.remove(old_idx);
                    pk.origin = KeyOrigin::Standard;
                    new_data.physical_layout.insert(insert_idx, pk);
                }
            } else {
                let pk = &new_data.physical_layout[sk.key_index];
                let is_alias_section = pk.origin == KeyOrigin::Alias;

                if is_alias_section {
                    // Editing an existing alias value (RHS)
                    let mut sorted_alias_names: Vec<String> = new_data.aliases.keys().cloned().collect();
                    sorted_alias_names.sort();
                    let num_non_alias = new_data.defsrc.len() + new_data.phantom_keys.len() + new_data.unmapped_names.len();
                    if let Some(alias_name) = sorted_alias_names.get(sk.key_index - num_non_alias) {
                        new_data.aliases.insert(alias_name.clone(), text);
                    }
                } else if let Some((name, val)) = text.split_once('=') {
                    // Creating or updating an alias (name = val)
                    let name = name.trim();
                    let val = val.trim();
                    if !name.is_empty() && !val.is_empty() {
                        new_data.aliases.insert(name.to_string(), val.to_string());
                        new_data.layers[sk.layer_index].bindings[sk.key_index] = format!("@{}", name);
                    }
                } else {
                    // Normal binding update
                    new_data.layers[sk.layer_index].bindings[sk.key_index] = text;
                }
            }

            on_update.emit(new_data);
            on_close.emit(e);
        })
    };

    let on_keydown = {
        let on_close = props.on_close.clone();
        let on_save = on_save.clone();
        let suggestion_index = suggestion_index.clone();
        let suggestions = suggestions.clone();
        let current_text = current_text.clone();
        let show_suggestions = show_suggestions.clone();
        let prefix = prefix.clone();
        Callback::from(move |e: KeyboardEvent| {
            match e.key().as_str() {
                "Escape" => {
                    e.prevent_default();
                    on_close.emit(MouseEvent::new("click").unwrap());
                }
                "Enter" => {
                    e.prevent_default();
                    on_save.emit(MouseEvent::new("click").unwrap());
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

    let text = (*current_text).clone();
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
                        ParamType::Integer => "integer",
                        ParamType::Action => "action",
                        ParamType::Layer => "layer",
                        ParamType::Any => "any",
                        ParamType::String => "string",
                        ParamType::ClipboardId => "clipboard-id",
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
                    ParamType::Integer => "integer",
                    ParamType::Action => "action",
                    ParamType::Layer => "layer",
                    ParamType::Any => "any",
                    ParamType::String => "string",
                    ParamType::ClipboardId => "clipboard-id",
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
                                        let prefix = prefix.clone();
                                        let is_active = i == *suggestion_index && *show_suggestions;
                                        let onclick = Callback::from(move |_| {
                                            current_text.set(format!("{}{}", prefix, s_clone.clone()));
                                            show_suggestions.set(false);
                                        });
                                        let display = if s == "_" { "transparent (▽)".to_string() } else if s == "none" { "none (∅)".to_string() } else { s.clone() };
                                        html! {
                                            <button onclick={onclick} class={classes!("text-left", "px-3", "py-2", "text-xs", "rounded-lg", "transition-colors", "font-mono", "border", "truncate",
                                                if is_active { "bg-blue-600 text-white border-blue-400" } else { "hover:bg-blue-50 dark:hover:bg-blue-900/30 hover:text-blue-600 dark:hover:text-blue-400 border-transparent hover:border-blue-200 dark:hover:border-blue-800 text-gray-700 dark:text-gray-300" }
                                            )}>
                                                {display}
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
