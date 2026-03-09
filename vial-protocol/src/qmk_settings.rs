//! QMK Settings definition table.
//!
//! Maps qsid → human-readable metadata (title, type, bit position, range, etc.).
//! Based on <https://github.com/drpngx/sval-keybard/blob/master/pages/js/qmk_settings.js>

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QmkSettingDef {
    pub qsid: u16,
    pub tab: &'static str,
    pub title: &'static str,
    pub field_type: FieldType,
    /// Byte width of the setting value stored in firmware.
    pub width: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    /// A single bit inside a multi-bit qsid. `bit` is the bit position.
    Boolean { bit: u8 },
    /// Integer with min/max range.
    Integer { min: u32, max: u32 },
    /// HSV colour (3 bytes: H, S, V).
    ColorHsv,
}

/// Static table of all known QMK setting fields.
pub static QMK_SETTINGS: &[QmkSettingDef] = &[
    // ---- Grave Escape ----
    QmkSettingDef {
        qsid: 1,
        tab: "Grave Escape",
        title: "Always send Escape if Alt is pressed",
        field_type: FieldType::Boolean { bit: 0 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 1,
        tab: "Grave Escape",
        title: "Always send Escape if Control is pressed",
        field_type: FieldType::Boolean { bit: 1 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 1,
        tab: "Grave Escape",
        title: "Always send Escape if GUI is pressed",
        field_type: FieldType::Boolean { bit: 2 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 1,
        tab: "Grave Escape",
        title: "Always send Escape if Shift is pressed",
        field_type: FieldType::Boolean { bit: 3 },
        width: 1,
    },
    // ---- Combo ----
    QmkSettingDef {
        qsid: 2,
        tab: "Combo",
        title: "Time out period for combos",
        field_type: FieldType::Integer { min: 0, max: 10000 },
        width: 2,
    },
    // ---- Auto Shift ----
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Enable",
        field_type: FieldType::Boolean { bit: 0 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Enable for modifiers",
        field_type: FieldType::Boolean { bit: 1 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Do not Auto Shift special keys",
        field_type: FieldType::Boolean { bit: 2 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Do not Auto Shift numeric keys",
        field_type: FieldType::Boolean { bit: 3 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Do not Auto Shift alpha characters",
        field_type: FieldType::Boolean { bit: 4 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Enable keyrepeat",
        field_type: FieldType::Boolean { bit: 5 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 3,
        tab: "Auto Shift",
        title: "Disable keyrepeat when timeout is exceeded",
        field_type: FieldType::Boolean { bit: 6 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 4,
        tab: "Auto Shift",
        title: "Timeout",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    // ---- One Shot Keys ----
    QmkSettingDef {
        qsid: 5,
        tab: "One Shot Keys",
        title: "Tapping this number of times holds the key until tapped once again",
        field_type: FieldType::Integer { min: 0, max: 50 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 6,
        tab: "One Shot Keys",
        title: "Time (in ms) before the one shot key is released",
        field_type: FieldType::Integer { min: 0, max: 60000 },
        width: 2,
    },
    // ---- Tap-Hold ----
    QmkSettingDef {
        qsid: 7,
        tab: "Tap-Hold",
        title: "Tapping Term",
        field_type: FieldType::Integer { min: 0, max: 10000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 8,
        tab: "Tap-Hold",
        title: "Permissive Hold",
        field_type: FieldType::Boolean { bit: 0 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 8,
        tab: "Tap-Hold",
        title: "Ignore Mod Tap Interrupt",
        field_type: FieldType::Boolean { bit: 1 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 8,
        tab: "Tap-Hold",
        title: "Tapping Force Hold",
        field_type: FieldType::Boolean { bit: 2 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 8,
        tab: "Tap-Hold",
        title: "Retro Tapping",
        field_type: FieldType::Boolean { bit: 3 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 18,
        tab: "Tap-Hold",
        title: "Tap Code Delay",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 19,
        tab: "Tap-Hold",
        title: "Tap Hold Caps Delay",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 20,
        tab: "Tap-Hold",
        title: "Tapping Toggle",
        field_type: FieldType::Integer { min: 0, max: 100 },
        width: 1,
    },
    // ---- Mouse Keys ----
    QmkSettingDef {
        qsid: 9,
        tab: "Mouse Keys",
        title: "Delay between pressing a movement key and cursor movement",
        field_type: FieldType::Integer { min: 0, max: 10000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 10,
        tab: "Mouse Keys",
        title: "Time between cursor movements in milliseconds",
        field_type: FieldType::Integer { min: 0, max: 10000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 11,
        tab: "Mouse Keys",
        title: "Step size",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 12,
        tab: "Mouse Keys",
        title: "Maximum cursor speed at which acceleration stops",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 13,
        tab: "Mouse Keys",
        title: "Time until maximum cursor speed is reached",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 14,
        tab: "Mouse Keys",
        title: "Delay between pressing a wheel key and wheel movement",
        field_type: FieldType::Integer { min: 0, max: 10000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 15,
        tab: "Mouse Keys",
        title: "Time between wheel movements",
        field_type: FieldType::Integer { min: 0, max: 10000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 16,
        tab: "Mouse Keys",
        title: "Maximum number of scroll steps per scroll action",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 17,
        tab: "Mouse Keys",
        title: "Time until maximum scroll speed is reached",
        field_type: FieldType::Integer { min: 0, max: 1000 },
        width: 2,
    },
    // ---- Magic ----
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap Caps Lock and Left Control",
        field_type: FieldType::Boolean { bit: 0 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Treat Caps Lock as Control",
        field_type: FieldType::Boolean { bit: 1 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap Left Alt and GUI",
        field_type: FieldType::Boolean { bit: 2 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap Right Alt and GUI",
        field_type: FieldType::Boolean { bit: 3 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Disable the GUI keys",
        field_type: FieldType::Boolean { bit: 4 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap ` and Escape",
        field_type: FieldType::Boolean { bit: 5 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap \\ and Backspace",
        field_type: FieldType::Boolean { bit: 6 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Enable N-key rollover",
        field_type: FieldType::Boolean { bit: 7 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap Left Control and GUI",
        field_type: FieldType::Boolean { bit: 8 },
        width: 4,
    },
    QmkSettingDef {
        qsid: 21,
        tab: "Magic",
        title: "Swap Right Control and GUI",
        field_type: FieldType::Boolean { bit: 9 },
        width: 4,
    },
    // ---- Sval (custom Svalboard settings) ----
    QmkSettingDef {
        qsid: 500,
        tab: "Sval",
        title: "Achordion mode",
        field_type: FieldType::Boolean { bit: 0 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 501,
        tab: "Sval",
        title: "Automouse",
        field_type: FieldType::Boolean { bit: 0 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 502,
        tab: "Sval",
        title: "Left mouse scroll",
        field_type: FieldType::Boolean { bit: 0 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 502,
        tab: "Sval",
        title: "Right mouse scroll",
        field_type: FieldType::Boolean { bit: 1 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 503,
        tab: "Sval",
        title: "Automouse timeout index",
        field_type: FieldType::Integer { min: 0, max: 4 },
        width: 1,
    },
    QmkSettingDef {
        qsid: 504,
        tab: "Sval",
        title: "Automouse timeout value (ms): 0",
        field_type: FieldType::Integer { min: 0, max: 60000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 505,
        tab: "Sval",
        title: "Automouse timeout value (ms): 1",
        field_type: FieldType::Integer { min: 0, max: 60000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 506,
        tab: "Sval",
        title: "Automouse timeout value (ms): 2",
        field_type: FieldType::Integer { min: 0, max: 60000 },
        width: 2,
    },
    QmkSettingDef {
        qsid: 507,
        tab: "Sval",
        title: "Automouse timeout value (ms): 3",
        field_type: FieldType::Integer { min: 0, max: 60000 },
        width: 2,
    },
    // ---- Sval colours ----
    QmkSettingDef {
        qsid: 508,
        tab: "Sval Colors",
        title: "Layer 0",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 509,
        tab: "Sval Colors",
        title: "Layer 1",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 510,
        tab: "Sval Colors",
        title: "Layer 2",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 511,
        tab: "Sval Colors",
        title: "Layer 3",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 512,
        tab: "Sval Colors",
        title: "Layer 4",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 513,
        tab: "Sval Colors",
        title: "Layer 5",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 514,
        tab: "Sval Colors",
        title: "Layer 6",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 515,
        tab: "Sval Colors",
        title: "Layer 7",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 516,
        tab: "Sval Colors",
        title: "Layer 8",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 517,
        tab: "Sval Colors",
        title: "Layer 9",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 518,
        tab: "Sval Colors",
        title: "Layer 10",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 519,
        tab: "Sval Colors",
        title: "Layer 11",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 520,
        tab: "Sval Colors",
        title: "Layer 12",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 521,
        tab: "Sval Colors",
        title: "Layer 13",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 522,
        tab: "Sval Colors",
        title: "Layer 14",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
    QmkSettingDef {
        qsid: 523,
        tab: "Sval Colors",
        title: "Layer 15",
        field_type: FieldType::ColorHsv,
        width: 3,
    },
];

/// Find all field definitions for a given qsid.
pub fn fields_for_qsid(qsid: u16) -> Vec<&'static QmkSettingDef> {
    QMK_SETTINGS.iter().filter(|d| d.qsid == qsid).collect()
}

/// Check whether a qsid has any known definition.
pub fn is_qsid_supported(qsid: u16) -> bool {
    QMK_SETTINGS.iter().any(|d| d.qsid == qsid)
}

/// Get all unique tab names, in definition order.
pub fn tab_names() -> Vec<&'static str> {
    let mut tabs = Vec::new();
    for def in QMK_SETTINGS {
        if !tabs.contains(&def.tab) {
            tabs.push(def.tab);
        }
    }
    tabs
}

/// Get the byte width for a qsid (all fields with the same qsid share the same width).
pub fn width_for_qsid(qsid: u16) -> u8 {
    QMK_SETTINGS
        .iter()
        .find(|d| d.qsid == qsid)
        .map(|d| d.width)
        .unwrap_or(1)
}

/// Read a boolean bit from a raw value buffer.
pub fn read_bool(value: &[u8], bit: u8) -> bool {
    let byte_idx = (bit / 8) as usize;
    let bit_idx = bit % 8;
    if byte_idx < value.len() {
        (value[byte_idx] >> bit_idx) & 1 != 0
    } else {
        false
    }
}

/// Write a boolean bit into a raw value buffer (in-place).
pub fn write_bool(value: &mut [u8], bit: u8, on: bool) {
    let byte_idx = (bit / 8) as usize;
    let bit_idx = bit % 8;
    if byte_idx < value.len() {
        if on {
            value[byte_idx] |= 1 << bit_idx;
        } else {
            value[byte_idx] &= !(1 << bit_idx);
        }
    }
}

/// Read an integer value from a raw LE byte buffer.
pub fn read_integer(value: &[u8], width: u8) -> u32 {
    match width {
        1 => value.first().copied().unwrap_or(0) as u32,
        2 => {
            let lo = value.first().copied().unwrap_or(0);
            let hi = value.get(1).copied().unwrap_or(0);
            u16::from_le_bytes([lo, hi]) as u32
        }
        _ => {
            let mut bytes = [0u8; 4];
            for (i, b) in value.iter().take(4).enumerate() {
                bytes[i] = *b;
            }
            u32::from_le_bytes(bytes)
        }
    }
}

/// Write an integer value into a raw LE byte buffer.
pub fn write_integer(value: &mut [u8], width: u8, num: u32) {
    match width {
        1 => {
            if !value.is_empty() {
                value[0] = num as u8;
            }
        }
        2 => {
            let bytes = (num as u16).to_le_bytes();
            if value.len() >= 2 {
                value[0] = bytes[0];
                value[1] = bytes[1];
            }
        }
        _ => {
            let bytes = num.to_le_bytes();
            for (i, b) in bytes.iter().enumerate() {
                if i < value.len() {
                    value[i] = *b;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tapping_term() {
        let fields = fields_for_qsid(7);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].title, "Tapping Term");
        assert_eq!(fields[0].tab, "Tap-Hold");
    }

    #[test]
    fn lookup_magic_bits() {
        let fields = fields_for_qsid(21);
        assert_eq!(fields.len(), 10);
        assert_eq!(fields[0].title, "Swap Caps Lock and Left Control");
    }

    #[test]
    fn bool_read_write() {
        let mut val = vec![0u8; 4];
        write_bool(&mut val, 7, true);
        assert!(read_bool(&val, 7));
        assert!(!read_bool(&val, 6));
        write_bool(&mut val, 9, true);
        assert!(read_bool(&val, 9));
        write_bool(&mut val, 7, false);
        assert!(!read_bool(&val, 7));
    }

    #[test]
    fn integer_read_write() {
        let mut val = vec![0u8; 2];
        write_integer(&mut val, 2, 200);
        assert_eq!(read_integer(&val, 2), 200);
    }

    #[test]
    fn tab_names_unique() {
        let tabs = tab_names();
        assert!(tabs.contains(&"Tap-Hold"));
        assert!(tabs.contains(&"Magic"));
        // no duplicates
        let mut deduped = tabs.clone();
        deduped.dedup();
        assert_eq!(tabs.len(), deduped.len());
    }
}
