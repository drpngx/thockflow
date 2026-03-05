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
        m
    };
}

pub fn format_keycode(code: &str) -> String {
    KEY_ALIASES.get(code).cloned().unwrap_or(code).to_string()
}
