use lazy_static::lazy_static;
use std::collections::HashMap;

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
        m.insert("C_MUTE", "Vol🔇");
        m.insert("C_VOL_UP", "Vol↑");
        m.insert("C_VOL_DN", "Vol↓");
        m.insert("C_PP", "Play");
        m.insert("LEFT", "←");
        m.insert("RIGHT", "→");
        m.insert("UP", "↑");
        m.insert("DOWN", "↓");
        m.insert("HOME", "Home");
        m.insert("END", "End");

        // Mouse Emulation
        m.insert("LCLK", "🖱️🅻");
        m.insert("RCLK", "🖱️🆁");
        m.insert("MCLK", "🖱️🅼");
        m.insert("MB4", "🖱️4");
        m.insert("MB5", "🖱️5");
        m.insert("MOVE_UP", "🖱️↑");
        m.insert("MOVE_DOWN", "🖱️↓");
        m.insert("MOVE_LEFT", "🖱️←");
        m.insert("MOVE_RIGHT", "🖱️→");
        m.insert("SCROLL_UP", "🖱️↑");
        m.insert("SCROLL_DOWN", "🖱️↓");
        m.insert("SCROLL_LEFT", "🖱️←");
        m.insert("SCROLL_RIGHT", "🖱️→");

        // Bluetooth
        m.insert("BT_CLR", "BT Clear");
        m.insert("BT_SEL", "BT Select");
        m.insert("BT_PRV", "BT Prev");
        m.insert("BT_PREV", "BT Prev");
        m.insert("BT_NXT", "BT Next");
        m.insert("BT_NEXT", "BT Next");
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

        // Browser / Media
        m.insert("C_AC_BACK", "Br←");
        m.insert("C_AC_FORWARD", "Br→");
        m.insert("K_MENU", "☰");

        // Number Row
        m.insert("N1", "1");
        m.insert("N2", "2");
        m.insert("N3", "3");
        m.insert("N4", "4");
        m.insert("N5", "5");
        m.insert("N6", "6");
        m.insert("N7", "7");
        m.insert("N8", "8");
        m.insert("N9", "9");
        m.insert("N0", "0");

        m
    };
}

pub fn format_keycode(code: &str) -> String {
    KEY_ALIASES.get(code).cloned().unwrap_or(code).to_string()
}

pub fn get_keycode_shifted(code: &str) -> Option<&'static str> {
    match code {
        "N1" => Some("!"),
        "N2" => Some("@"),
        "N3" => Some("#"),
        "N4" => Some("$"),
        "N5" => Some("%"),
        "N6" => Some("^"),
        "N7" => Some("&"),
        "N8" => Some("*"),
        "N9" => Some("("),
        "N0" => Some(")"),
        "MINUS" => Some("_"),
        "EQUAL" => Some("+"),
        "GRAVE" => Some("~"),
        "LBKT" => Some("{"),
        "RBKT" => Some("}"),
        "BSLH" => Some("|"),
        "SEMI" => Some(":"),
        "SQT" => Some("\""),
        "COMMA" => Some("<"),
        "DOT" => Some(">"),
        "SLASH" => Some("?"),
        _ => None,
    }
}

pub fn is_regular_key(code: &str) -> bool {
    is_plain_key(code) || is_modifier(code)
}

pub fn is_plain_key(code: &str) -> bool {
    if code.len() == 1 && code.chars().next().unwrap().is_ascii_alphabetic() {
        return true;
    }
    if code.starts_with('N') && code.len() == 2 && code.chars().nth(1).unwrap().is_ascii_digit() {
        return true;
    }
    let regular_aliases = [
        "GRAVE",
        "SEMI",
        "SQT",
        "SLASH",
        "BSPC",
        "LBKT",
        "RBKT",
        "MINUS",
        "EQUAL",
        "COMMA",
        "DOT",
        "BSLH",
        "ESC",
        "SPACE",
        "ENTER",
        "TAB",
        "LEFT",
        "RIGHT",
        "UP",
        "DOWN",
        "HOME",
        "END",
        "PG_UP",
        "PG_DN",
        "DEL",
        "INS",
        "C_AC_BACK",
        "C_AC_FORWARD",
        "C_VOL_UP",
        "C_VOL_DN",
        "C_MUTE",
        "K_MENU",
    ];
    if regular_aliases.contains(&code) {
        return true;
    }
    if code.starts_with('F') && code.len() > 1 && code[1..].parse::<u8>().is_ok() {
        return true;
    }
    false
}

pub fn to_zmk_keycode(key: &str) -> Option<String> {
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase().to_string());
        }
        if c.is_ascii_digit() {
            return Some(format!("N{}", c));
        }
        return match c {
            ' ' => Some("SPACE".into()),
            ',' => Some("COMMA".into()),
            '.' => Some("DOT".into()),
            '/' => Some("SLASH".into()),
            ';' => Some("SEMI".into()),
            '\'' => Some("SQT".into()),
            '[' => Some("LBKT".into()),
            ']' => Some("RBKT".into()),
            '\\' => Some("BSLH".into()),
            '-' => Some("MINUS".into()),
            '=' => Some("EQUAL".into()),
            '`' => Some("GRAVE".into()),
            _ => None,
        };
    }
    match key {
        "Enter" => Some("ENTER".into()),
        "Escape" => Some("ESC".into()),
        "Backspace" => Some("BSPC".into()),
        "Tab" => Some("TAB".into()),
        "ArrowLeft" => Some("LEFT".into()),
        "ArrowRight" => Some("RIGHT".into()),
        "ArrowUp" => Some("UP".into()),
        "ArrowDown" => Some("DOWN".into()),
        "Delete" => Some("DEL".into()),
        "Home" => Some("HOME".into()),
        "End" => Some("END".into()),
        "PageUp" => Some("PG_UP".into()),
        "PageDown" => Some("PG_DN".into()),
        "Insert" => Some("INS".into()),
        _ if key.starts_with('F') && key.len() > 1 => Some(key.to_ascii_uppercase()),
        _ => None,
    }
}

pub fn is_modifier(code: &str) -> bool {
    code.starts_with("L")
        && (code.contains("SHFT")
            || code.contains("CTRL")
            || code.contains("ALT")
            || code.contains("GUI"))
        || code.starts_with("R")
            && (code.contains("SHFT")
                || code.contains("CTRL")
                || code.contains("ALT")
                || code.contains("GUI"))
        || code.starts_with("MOD_")
}
