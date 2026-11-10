//! QMK dynamic macro buffer: NUL-separated macros of text and `SS_QMK_PREFIX`
//! actions, as Keybard reads and writes them.

#[cfg(feature = "std")]
use super::keycode_names::KeycodeNames;

const QMK_EXT_ID: u8 = 1;
const ACTION_TAP: u8 = 1;
const ACTION_DOWN: u8 = 2;
const ACTION_UP: u8 = 3;
const ACTION_DELAY: u8 = 4;
const ACTION_TAP_16: u8 = 5;
const ACTION_DOWN_16: u8 = 6;
const ACTION_UP_16: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroAction {
    Tap(u16),
    Down(u16),
    Up(u16),
    Delay(u32),
    Text(String),
}

impl MacroAction {
    /// `.svil` form: `["tap", "KC_A"]`, `["delay", 100]`, `["text", "hi"]`.
    #[cfg(feature = "std")]
    pub fn to_json(&self, names: &KeycodeNames) -> serde_json::Value {
        use serde_json::json;
        match self {
            MacroAction::Tap(k) => json!(["tap", names.stringify(*k)]),
            MacroAction::Down(k) => json!(["down", names.stringify(*k)]),
            MacroAction::Up(k) => json!(["up", names.stringify(*k)]),
            MacroAction::Delay(ms) => json!(["delay", ms]),
            MacroAction::Text(t) => json!(["text", t]),
        }
    }

    /// Read one `.svil` action group `[type, value, value...]`.
    #[cfg(feature = "std")]
    pub fn from_json(v: &serde_json::Value, names: &KeycodeNames) -> Result<Vec<MacroAction>, String> {
        let arr = v.as_array().ok_or("macro action must be a list")?;
        let kind = arr.first().and_then(|k| k.as_str()).ok_or("macro action needs a type")?;
        let mut out = Vec::new();
        for value in &arr[1..] {
            let key = || -> Result<u16, String> {
                match value {
                    serde_json::Value::String(s) => names.parse(s).ok_or_else(|| format!("unknown keycode {s}")),
                    serde_json::Value::Number(n) => n.as_u64().map(|n| n as u16).ok_or("bad keycode".into()),
                    _ => Err("bad keycode".into()),
                }
            };
            out.push(match kind {
                "tap" => MacroAction::Tap(key()?),
                "down" => MacroAction::Down(key()?),
                "up" => MacroAction::Up(key()?),
                "delay" => MacroAction::Delay(value.as_u64().ok_or("delay must be a number")? as u32),
                "text" => MacroAction::Text(value.as_str().ok_or("text must be a string")?.to_string()),
                other => return Err(format!("unknown macro action {other}")),
            });
        }
        Ok(out)
    }
}

/// Split the first `count` NUL-terminated macros out of the buffer.
pub fn split(buffer: &[u8], count: usize) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut offset = 0;
    while out.len() < count && offset < buffer.len() {
        let start = offset;
        while offset < buffer.len() && buffer[offset] != 0 {
            offset += 1;
        }
        out.push(&buffer[start..offset]);
        offset += 1;
    }
    out
}

/// Parse one macro (without its terminator).
pub fn parse(raw: &[u8]) -> Vec<MacroAction> {
    let mut actions = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            0 => break,
            QMK_EXT_ID => {
                let Some(&kind) = raw.get(i + 1) else { break };
                match kind {
                    ACTION_TAP | ACTION_DOWN | ACTION_UP => {
                        let Some(&k) = raw.get(i + 2) else { break };
                        let k = k as u16;
                        actions.push(match kind {
                            ACTION_TAP => MacroAction::Tap(k),
                            ACTION_DOWN => MacroAction::Down(k),
                            _ => MacroAction::Up(k),
                        });
                        i += 3;
                    }
                    ACTION_DELAY => {
                        if i + 3 >= raw.len() {
                            break;
                        }
                        let ms = (raw[i + 2] as u32).saturating_sub(1) + (raw[i + 3] as u32).saturating_sub(1) * 255;
                        actions.push(MacroAction::Delay(ms));
                        i += 4;
                    }
                    ACTION_TAP_16 | ACTION_DOWN_16 | ACTION_UP_16 => {
                        if i + 3 >= raw.len() {
                            break;
                        }
                        let k = u16::from_le_bytes([raw[i + 2], raw[i + 3]]);
                        actions.push(match kind {
                            ACTION_TAP_16 => MacroAction::Tap(k),
                            ACTION_DOWN_16 => MacroAction::Down(k),
                            _ => MacroAction::Up(k),
                        });
                        i += 4;
                    }
                    _ => break,
                }
            }
            _ => {
                let start = i;
                while i < raw.len() && raw[i] > 4 {
                    i += 1;
                }
                if i == start {
                    break;
                }
                actions.push(MacroAction::Text(String::from_utf8_lossy(&raw[start..i]).into_owned()));
            }
        }
    }
    actions
}

/// Serialize macros into a buffer (each terminated by NUL).
pub fn dump(macros: &[Vec<MacroAction>]) -> Vec<u8> {
    let mut out = Vec::new();
    for actions in macros {
        for a in actions {
            match a {
                MacroAction::Text(t) => out.extend_from_slice(t.as_bytes()),
                MacroAction::Delay(ms) => {
                    out.extend_from_slice(&[QMK_EXT_ID, ACTION_DELAY, (ms % 255 + 1) as u8, (ms / 255 + 1) as u8])
                }
                MacroAction::Tap(k) | MacroAction::Down(k) | MacroAction::Up(k) => {
                    if *k == 0 {
                        continue; // would terminate the macro early
                    }
                    let (short, long) = match a {
                        MacroAction::Tap(_) => (ACTION_TAP, ACTION_TAP_16),
                        MacroAction::Down(_) => (ACTION_DOWN, ACTION_DOWN_16),
                        _ => (ACTION_UP, ACTION_UP_16),
                    };
                    if *k >= 0x100 {
                        let b = k.to_le_bytes();
                        out.extend_from_slice(&[QMK_EXT_ID, long, b[0], b[1]]);
                    } else {
                        out.extend_from_slice(&[QMK_EXT_ID, short, *k as u8]);
                    }
                }
            }
        }
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let macros = vec![
            vec![MacroAction::Text("hi".into()), MacroAction::Tap(0x28), MacroAction::Delay(300)],
            vec![],
            vec![MacroAction::Down(0xE1), MacroAction::Tap(0x0204), MacroAction::Up(0xE1)],
        ];
        let buf = dump(&macros);
        let parsed: Vec<_> = split(&buf, 3).into_iter().map(parse).collect();
        assert_eq!(parsed, macros);
    }
}
