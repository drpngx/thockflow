use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref KEY_ALIASES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("GRAVE", "`");
        m.insert("SEMI", ";");
        m.insert("SQT", "'");
        m.insert("SLASH", "/");
        m.insert("BSPC", "Bksp");
        m.insert("LSHFT", "Shift");
        m.insert("RSHFT", "Shift");
        m.insert("LCTRL", "Ctrl");
        m.insert("RCTRL", "Ctrl");
        m.insert("LALT", "Alt");
        m.insert("RALT", "Alt");
        m.insert("LGUI", "Gui");
        m.insert("RGUI", "Gui");
        m.insert("LBKT", "[");
        m.insert("RBKT", "]");
        m.insert("LPAR", "(");
        m.insert("RPAR", ")");
        m.insert("LBRC", "{");
        m.insert("RBRC", "}");
        m.insert("EXCL", "!");
        m.insert("AT", "@");
        m.insert("HASH", "#");
        m.insert("DOLLAR", "$");
        m.insert("PERCENT", "%");
        m.insert("CARET", "^");
        m.insert("AMPS", "&");
        m.insert("ASTRK", "*");
        m.insert("MINUS", "-");
        m.insert("UNDER", "_");
        m.insert("PLUS", "+");
        m.insert("EQUAL", "=");
        m.insert("COMMA", ",");
        m.insert("DOT", ".");
        m.insert("PIPE", "|");
        m.insert("BSLH", "\\");
        m.insert("TILDE", "~");
        m.insert("LT", "<");
        m.insert("GT", ">");
        m.insert("INS", "Ins");
        m.insert("DEL", "Del");
        m.insert("PG_UP", "PgUp");
        m.insert("PG_DN", "PgDn");
        m.insert("PRINTSCREEN", "PrtSc");
        m.insert("ESC", "Esc");
        m.insert("SPACE", "Space");
        m.insert("ENTER", "Enter");
        m.insert("TAB", "Tab");
        m.insert("C_MUTE", "Mute");
        m.insert("C_VOL_UP", "Vol+");
        m.insert("C_VOL_DN", "Vol-");
        m.insert("C_PP", "Play");
        m.insert("LEFT", "←");
        m.insert("RIGHT", "→");
        m.insert("UP", "↑");
        m.insert("DOWN", "↓");
        m.insert("HOME", "Home");
        m.insert("END", "End");
        
        // Mouse Emulation
        m.insert("LCLK", "L-Click");
        m.insert("RCLK", "R-Click");
        m.insert("MCLK", "M-Click");
        m.insert("MB4", "Mouse 4");
        m.insert("MB5", "Mouse 5");
        m.insert("MOVE_UP", "Move ↑");
        m.insert("MOVE_DOWN", "Move ↓");
        m.insert("MOVE_LEFT", "Move ←");
        m.insert("MOVE_RIGHT", "Move →");
        m.insert("SCROLL_UP", "Scroll ↑");
        m.insert("SCROLL_DOWN", "Scroll ↓");
        
        // Bluetooth
        m.insert("BT_CLR", "BT Clear");
        m.insert("BT_SEL", "BT Select");
        m.insert("BT_PRV", "BT Prev");
        m.insert("BT_NXT", "BT Next");
        m.insert("BT_CLR_ALL", "BT Clear All");
        m.insert("BT_DISC", "BT Disconnect");
        
        // RGB Underglow
        m.insert("RGB_TOG", "RGB Toggle");
        m.insert("RGB_HUI", "RGB Hue+");
        m.insert("RGB_HUD", "RGB Hue-");
        m.insert("RGB_SAI", "RGB Sat+");
        m.insert("RGB_SAD", "RGB Sat-");
        m.insert("RGB_BRI", "RGB Bri+");
        m.insert("RGB_BRD", "RGB Bri-");
        m.insert("RGB_SPI", "RGB Spd+");
        m.insert("RGB_SPD", "RGB Spd-");
        m.insert("RGB_EFF", "RGB Eff");
        m.insert("RGB_COLOR_HSB", "RGB Color");
        
        // Outputs
        m.insert("OUT_TOG", "Out Toggle");
        m.insert("OUT_USB", "Out USB");
        m.insert("OUT_BLE", "Out BLE");
        
        m
    };
}

pub fn format_keycode(code: &str) -> String {
    KEY_ALIASES.get(code).cloned().unwrap_or(code).to_string()
}

pub fn is_modifier(code: &str) -> bool {
    code.starts_with("L") && (code.contains("SHFT") || code.contains("CTRL") || code.contains("ALT") || code.contains("GUI")) ||
    code.starts_with("R") && (code.contains("SHFT") || code.contains("CTRL") || code.contains("ALT") || code.contains("GUI")) ||
    code.starts_with("MOD_")
}
