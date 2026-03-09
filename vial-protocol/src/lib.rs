//! Vial/VIA keyboard protocol implementation.
//!
//! Command IDs match the canonical sources:
//!   - VIA:  <https://github.com/the-via/app/blob/master/src/common/types.ts>
//!   - Vial: <https://github.com/drpngx/sval-vial-gui/blob/web/src/main/python/protocol/constants.py>

pub mod keycodes;
pub mod qmk_settings;

use serde::{Deserialize, Serialize};

// Protocol constants
pub const MSG_LEN: usize = 32;
pub const RAW_HID_USAGE_PAGE: u16 = 0xFF60;
pub const RAW_HID_USAGE_ID: u16 = 0x61;
/// How many payload bytes fit in a single buffer-fetch packet.
pub const BUFFER_FETCH_CHUNK: usize = 28;
/// Vial protocol version that introduced QMK settings support.
pub const VIAL_PROTOCOL_QMK_SETTINGS: u32 = 4;

// ---------------------------------------------------------------------------
// VIA command IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ViaCommand {
    GetProtocolVersion = 0x01,
    GetKeyboardValue = 0x02,
    SetKeyboardValue = 0x03,
    DynamicKeymapGetKeycode = 0x04,
    DynamicKeymapSetKeycode = 0x05,
    // 0x06 = dynamic_keymap_reset (not commonly used)
    LightingSetValue = 0x07,
    LightingGetValue = 0x08,
    LightingSave = 0x09,
    EepromReset = 0x0A,
    BootloaderJump = 0x0B,
    DynamicKeymapMacroGetCount = 0x0C,
    DynamicKeymapMacroGetBufferSize = 0x0D,
    DynamicKeymapMacroGetBuffer = 0x0E,
    DynamicKeymapMacroSetBuffer = 0x0F,
    // 0x10 = dynamic_keymap_macro_reset
    DynamicKeymapGetLayerCount = 0x11,
    DynamicKeymapGetBuffer = 0x12,
    DynamicKeymapSetBuffer = 0x13,
    Vial = 0xFE,
}

// ---------------------------------------------------------------------------
// Vial sub-command IDs (sent after ViaCommand::Vial = 0xFE)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VialCommand {
    GetKeyboardId = 0x00,
    GetSize = 0x01,
    GetDefinition = 0x02,
    GetEncoder = 0x03,
    SetEncoder = 0x04,
    GetUnlockStatus = 0x05,
    UnlockStart = 0x06,
    UnlockPoll = 0x07,
    Lock = 0x08,
    QmkSettingsQuery = 0x09,
    QmkSettingsGet = 0x0A,
    QmkSettingsSet = 0x0B,
    QmkSettingsReset = 0x0C,
    DynamicEntryOp = 0x0D,
}

// ---------------------------------------------------------------------------
// VIA keyboard value sub-IDs (used with GetKeyboardValue/SetKeyboardValue)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyboardValue {
    Uptime = 0x01,
    LayoutOptions = 0x02,
    SwitchMatrixState = 0x03,
    FirmwareVersion = 0x04,
    DeviceIndication = 0x05,
}

// ---------------------------------------------------------------------------
// Dynamic entry sub-ops (used with VialCommand::DynamicEntryOp)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DynamicEntryOp {
    GetNumberOfEntries = 0x00,
    TapDanceGet = 0x01,
    TapDanceSet = 0x02,
    ComboGet = 0x03,
    ComboSet = 0x04,
    KeyOverrideGet = 0x05,
    KeyOverrideSet = 0x06,
}

// ---------------------------------------------------------------------------
// Message builder
// ---------------------------------------------------------------------------

/// A 32-byte HID report buffer.
#[derive(Clone)]
pub struct VialMessage {
    pub data: [u8; MSG_LEN],
}

impl VialMessage {
    pub fn new() -> Self {
        VialMessage {
            data: [0u8; MSG_LEN],
        }
    }

    /// Create a VIA command message.
    pub fn via(cmd: ViaCommand) -> Self {
        let mut m = Self::new();
        m.data[0] = cmd as u8;
        m
    }

    /// Create a Vial sub-command message (prefixed with 0xFE).
    pub fn vial(cmd: VialCommand) -> Self {
        let mut m = Self::new();
        m.data[0] = ViaCommand::Vial as u8;
        m.data[1] = cmd as u8;
        m
    }

    // -- VIA helpers --------------------------------------------------------

    pub fn get_protocol_version() -> Self {
        Self::via(ViaCommand::GetProtocolVersion)
    }

    pub fn get_layer_count() -> Self {
        Self::via(ViaCommand::DynamicKeymapGetLayerCount)
    }

    pub fn get_keycode(layer: u8, row: u8, col: u8) -> Self {
        let mut m = Self::via(ViaCommand::DynamicKeymapGetKeycode);
        m.data[1] = layer;
        m.data[2] = row;
        m.data[3] = col;
        m
    }

    pub fn set_keycode(layer: u8, row: u8, col: u8, keycode: u16) -> Self {
        let mut m = Self::via(ViaCommand::DynamicKeymapSetKeycode);
        m.data[1] = layer;
        m.data[2] = row;
        m.data[3] = col;
        let [hi, lo] = keycode.to_be_bytes();
        m.data[4] = hi;
        m.data[5] = lo;
        m
    }

    /// Bulk-read keymap buffer. Offset and size are big-endian per VIA protocol.
    pub fn get_keymap_buffer(offset: u16, size: u8) -> Self {
        let mut m = Self::via(ViaCommand::DynamicKeymapGetBuffer);
        let [hi, lo] = offset.to_be_bytes();
        m.data[1] = hi;
        m.data[2] = lo;
        m.data[3] = size;
        m
    }

    pub fn set_keymap_buffer(offset: u16, payload: &[u8]) -> Self {
        let mut m = Self::via(ViaCommand::DynamicKeymapSetBuffer);
        let [hi, lo] = offset.to_be_bytes();
        m.data[1] = hi;
        m.data[2] = lo;
        let len = payload.len().min(MSG_LEN - 4);
        m.data[3] = len as u8;
        m.data[4..4 + len].copy_from_slice(&payload[..len]);
        m
    }

    pub fn get_keyboard_value(val: KeyboardValue) -> Self {
        let mut m = Self::via(ViaCommand::GetKeyboardValue);
        m.data[1] = val as u8;
        m
    }

    pub fn get_switch_matrix_state() -> Self {
        Self::get_keyboard_value(KeyboardValue::SwitchMatrixState)
    }

    // -- Vial helpers -------------------------------------------------------

    pub fn get_keyboard_id() -> Self {
        Self::vial(VialCommand::GetKeyboardId)
    }

    /// Get the size (in bytes) of the compressed definition payload.
    pub fn get_size() -> Self {
        Self::vial(VialCommand::GetSize)
    }

    /// Get a definition block. `block` is a 0-based page index, little-endian u32.
    pub fn get_definition(block: u32) -> Self {
        let mut m = Self::vial(VialCommand::GetDefinition);
        let bytes = block.to_le_bytes();
        m.data[2] = bytes[0];
        m.data[3] = bytes[1];
        m.data[4] = bytes[2];
        m.data[5] = bytes[3];
        m
    }

    pub fn get_unlock_status() -> Self {
        Self::vial(VialCommand::GetUnlockStatus)
    }

    pub fn unlock_start() -> Self {
        Self::vial(VialCommand::UnlockStart)
    }

    pub fn unlock_poll() -> Self {
        Self::vial(VialCommand::UnlockPoll)
    }

    pub fn lock() -> Self {
        Self::vial(VialCommand::Lock)
    }

    /// Query supported QMK setting IDs starting from `cursor`.
    /// The firmware returns LE16 qsid values; 0xFFFF = end sentinel.
    /// Pass 0 for the first page, then `max(returned qsids)` for subsequent pages.
    pub fn qmk_settings_query(cursor: u16) -> Self {
        let mut m = Self::vial(VialCommand::QmkSettingsQuery);
        let [lo, hi] = cursor.to_le_bytes();
        m.data[2] = lo;
        m.data[3] = hi;
        m
    }

    /// Read the current value of a QMK setting.
    pub fn qmk_settings_get(qsid: u16) -> Self {
        let mut m = Self::vial(VialCommand::QmkSettingsGet);
        let [lo, hi] = qsid.to_le_bytes();
        m.data[2] = lo;
        m.data[3] = hi;
        m
    }

    /// Write a QMK setting value.
    pub fn qmk_settings_set(qsid: u16, value: &[u8]) -> Self {
        let mut m = Self::vial(VialCommand::QmkSettingsSet);
        let [lo, hi] = qsid.to_le_bytes();
        m.data[2] = lo;
        m.data[3] = hi;
        let len = value.len().min(MSG_LEN - 4);
        m.data[4..4 + len].copy_from_slice(&value[..len]);
        m
    }

    pub fn qmk_settings_reset() -> Self {
        Self::vial(VialCommand::QmkSettingsReset)
    }

    /// Dynamic entry operation (tap-dance, combo, key-override).
    pub fn dynamic_entry_op(op: DynamicEntryOp, index: u8) -> Self {
        let mut m = Self::vial(VialCommand::DynamicEntryOp);
        m.data[2] = op as u8;
        m.data[3] = index;
        m
    }

    pub fn get_encoder(layer: u8, index: u8) -> Self {
        let mut m = Self::vial(VialCommand::GetEncoder);
        m.data[2] = layer;
        m.data[3] = index;
        m
    }

    pub fn set_encoder(layer: u8, index: u8, direction: u8, keycode: u16) -> Self {
        let mut m = Self::vial(VialCommand::SetEncoder);
        m.data[2] = layer;
        m.data[3] = index;
        m.data[4] = direction;
        let [hi, lo] = keycode.to_be_bytes();
        m.data[5] = hi;
        m.data[6] = lo;
        m
    }

    /// Raw bytes for sending over HID.
    pub fn as_bytes(&self) -> &[u8; MSG_LEN] {
        &self.data
    }
}

impl Default for VialMessage {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Response parsers
// ---------------------------------------------------------------------------

/// Parse VIA protocol version from a GetProtocolVersion response.
/// Response format: [cmd, version_hi, version_lo, ...]
pub fn parse_protocol_version(response: &[u8; MSG_LEN]) -> u16 {
    u16::from_be_bytes([response[1], response[2]])
}

/// Parse layer count from a DynamicKeymapGetLayerCount response.
pub fn parse_layer_count(response: &[u8; MSG_LEN]) -> u8 {
    response[1]
}

/// Parse a single keycode from a DynamicKeymapGetKeycode response.
pub fn parse_keycode(response: &[u8; MSG_LEN]) -> u16 {
    u16::from_be_bytes([response[4], response[5]])
}

/// Parse keyboard ID response → (vial_protocol_version, keyboard_uid).
/// Format: [vial_proto LE32] [uid LE64]
pub fn parse_keyboard_id(response: &[u8; MSG_LEN]) -> (u32, u64) {
    let version = u32::from_le_bytes([response[0], response[1], response[2], response[3]]);
    let uid = u64::from_le_bytes([
        response[4],
        response[5],
        response[6],
        response[7],
        response[8],
        response[9],
        response[10],
        response[11],
    ]);
    (version, uid)
}

/// Parse definition size response → total bytes (LE32).
pub fn parse_definition_size(response: &[u8; MSG_LEN]) -> u32 {
    u32::from_le_bytes([response[0], response[1], response[2], response[3]])
}

/// Parse unlock status → (unlocked, unlock_in_progress).
pub fn parse_unlock_status(response: &[u8; MSG_LEN]) -> (bool, bool) {
    (response[0] != 0, response[1] != 0)
}

// ---------------------------------------------------------------------------
// QMK Settings query response parsing
// ---------------------------------------------------------------------------

/// Parse a `qmk_settings_query` response.
/// The firmware packs LE16 qsid values; 0xFFFF = sentinel.
/// Returns the set of supported qsids found in this response page.
pub fn parse_qmk_settings_query(response: &[u8; MSG_LEN]) -> Vec<u16> {
    let mut out = Vec::new();
    let mut off = 0;
    while off + 2 <= MSG_LEN {
        let qsid = u16::from_le_bytes([response[off], response[off + 1]]);
        if qsid == 0xFFFF {
            break;
        }
        out.push(qsid);
        off += 2;
    }
    out
}

/// Parse a `qmk_settings_get` response.
/// Response[0] is status (0 = ok), value bytes start at [1].
pub fn parse_qmk_settings_get(response: &[u8; MSG_LEN], size: usize) -> (u8, Vec<u8>) {
    let status = response[0];
    let n = size.min(MSG_LEN - 1);
    (status, response[1..1 + n].to_vec())
}

// ---------------------------------------------------------------------------
// Switch matrix test
// ---------------------------------------------------------------------------

/// Parse the switch-matrix state bitfield returned by
/// `GetKeyboardValue(SwitchMatrixState)`.
///
/// The payload starts at `response[2]`. Each bit represents one switch
/// position, packed row-major: byte `row*ceil(cols/8) + col/8`, bit `col%8`.
pub fn parse_matrix_state(response: &[u8; MSG_LEN], rows: u8, cols: u8) -> Vec<Vec<bool>> {
    let cols_bytes = ((cols as usize) + 7) / 8;
    let payload = &response[2..];
    let mut matrix = vec![vec![false; cols as usize]; rows as usize];
    for r in 0..rows as usize {
        for c in 0..cols as usize {
            let byte_idx = r * cols_bytes + c / 8;
            let bit_idx = c % 8;
            if byte_idx < payload.len() {
                matrix[r][c] = (payload[byte_idx] >> bit_idx) & 1 != 0;
            }
        }
    }
    matrix
}

// ---------------------------------------------------------------------------
// Keyboard definition (Vial JSON, typically LZMA-compressed in firmware)
// ---------------------------------------------------------------------------

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VialDefinition {
    pub name: Option<String>,
    #[serde(rename = "vendorId")]
    pub vendor_id: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
    pub matrix: MatrixSize,
    pub layouts: VialLayouts,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixSize {
    pub rows: u8,
    pub cols: u8,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VialLayouts {
    pub keymap: Vec<Vec<VialKeyDef>>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VialKeyDef {
    Label(String),
    Options(serde_json::Value),
}

// ---------------------------------------------------------------------------
// Tap-dance / combo helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapDanceEntry {
    pub on_tap: u16,
    pub on_hold: u16,
    pub on_double_tap: u16,
    pub on_tap_hold: u16,
    pub tapping_term: u16,
}

impl TapDanceEntry {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 10 {
            return None;
        }
        Some(Self {
            on_tap: u16::from_le_bytes([data[0], data[1]]),
            on_hold: u16::from_le_bytes([data[2], data[3]]),
            on_double_tap: u16::from_le_bytes([data[4], data[5]]),
            on_tap_hold: u16::from_le_bytes([data[6], data[7]]),
            tapping_term: u16::from_le_bytes([data[8], data[9]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; 10] {
        let mut out = [0u8; 10];
        out[0..2].copy_from_slice(&self.on_tap.to_le_bytes());
        out[2..4].copy_from_slice(&self.on_hold.to_le_bytes());
        out[4..6].copy_from_slice(&self.on_double_tap.to_le_bytes());
        out[6..8].copy_from_slice(&self.on_tap_hold.to_le_bytes());
        out[8..10].copy_from_slice(&self.tapping_term.to_le_bytes());
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComboEntry {
    pub keys: [u16; 4],
    pub output: u16,
}

impl ComboEntry {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 10 {
            return None;
        }
        Some(Self {
            keys: [
                u16::from_le_bytes([data[0], data[1]]),
                u16::from_le_bytes([data[2], data[3]]),
                u16::from_le_bytes([data[4], data[5]]),
                u16::from_le_bytes([data[6], data[7]]),
            ],
            output: u16::from_le_bytes([data[8], data[9]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; 10] {
        let mut out = [0u8; 10];
        for (i, k) in self.keys.iter().enumerate() {
            out[i * 2..i * 2 + 2].copy_from_slice(&k.to_le_bytes());
        }
        out[8..10].copy_from_slice(&self.output.to_le_bytes());
        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn via_command_ids_match_reference() {
        assert_eq!(ViaCommand::GetProtocolVersion as u8, 0x01);
        assert_eq!(ViaCommand::DynamicKeymapGetLayerCount as u8, 0x11);
        assert_eq!(ViaCommand::DynamicKeymapGetBuffer as u8, 0x12);
        assert_eq!(ViaCommand::DynamicKeymapSetBuffer as u8, 0x13);
    }

    #[test]
    fn vial_command_ids_match_reference() {
        assert_eq!(VialCommand::GetKeyboardId as u8, 0x00);
        assert_eq!(VialCommand::GetSize as u8, 0x01);
        assert_eq!(VialCommand::GetDefinition as u8, 0x02);
        assert_eq!(VialCommand::GetEncoder as u8, 0x03);
        assert_eq!(VialCommand::SetEncoder as u8, 0x04);
        assert_eq!(VialCommand::GetUnlockStatus as u8, 0x05);
        assert_eq!(VialCommand::UnlockStart as u8, 0x06);
        assert_eq!(VialCommand::UnlockPoll as u8, 0x07);
        assert_eq!(VialCommand::Lock as u8, 0x08);
        assert_eq!(VialCommand::QmkSettingsQuery as u8, 0x09);
        assert_eq!(VialCommand::QmkSettingsGet as u8, 0x0A);
        assert_eq!(VialCommand::QmkSettingsSet as u8, 0x0B);
        assert_eq!(VialCommand::QmkSettingsReset as u8, 0x0C);
        assert_eq!(VialCommand::DynamicEntryOp as u8, 0x0D);
    }

    #[test]
    fn roundtrip_get_keycode() {
        let m = VialMessage::get_keycode(2, 3, 4);
        assert_eq!(m.data[0], 0x04);
        assert_eq!(m.data[1], 2);
        assert_eq!(m.data[2], 3);
        assert_eq!(m.data[3], 4);
    }

    #[test]
    fn roundtrip_set_keycode() {
        let m = VialMessage::set_keycode(1, 0, 5, 0x0028);
        assert_eq!(m.data[0], 0x05);
        assert_eq!(u16::from_be_bytes([m.data[4], m.data[5]]), 0x0028);
    }

    #[test]
    fn get_definition_uses_le32_block() {
        let m = VialMessage::get_definition(3);
        assert_eq!(m.data[0], 0xFE);
        assert_eq!(m.data[1], 0x02); // GetDefinition
        assert_eq!(m.data[2], 3); // block LE32
        assert_eq!(m.data[3], 0);
        assert_eq!(m.data[4], 0);
        assert_eq!(m.data[5], 0);
    }

    #[test]
    fn get_size_command() {
        let m = VialMessage::get_size();
        assert_eq!(m.data[0], 0xFE);
        assert_eq!(m.data[1], 0x01); // GetSize
    }

    #[test]
    fn qmk_settings_query_uses_le16() {
        let m = VialMessage::qmk_settings_query(0x1234);
        assert_eq!(m.data[0], 0xFE);
        assert_eq!(m.data[1], 0x09); // QmkSettingsQuery
        assert_eq!(m.data[2], 0x34); // LE16 low
        assert_eq!(m.data[3], 0x12); // LE16 high
    }

    #[test]
    fn parse_version() {
        let mut resp = [0u8; MSG_LEN];
        resp[1] = 0x00;
        resp[2] = 0x09; // version 9
        assert_eq!(parse_protocol_version(&resp), 9);
    }

    #[test]
    fn parse_layers() {
        let mut resp = [0u8; MSG_LEN];
        resp[1] = 16;
        assert_eq!(parse_layer_count(&resp), 16);
    }

    #[test]
    fn parse_def_size() {
        let mut resp = [0u8; MSG_LEN];
        resp[0] = 0x00;
        resp[1] = 0x10;
        resp[2] = 0x00;
        resp[3] = 0x00;
        assert_eq!(parse_definition_size(&resp), 0x1000);
    }

    #[test]
    fn settings_query_sentinel() {
        let resp = [0xFF; MSG_LEN];
        let qsids = parse_qmk_settings_query(&resp);
        assert!(qsids.is_empty());
    }

    #[test]
    fn settings_query_entries() {
        let mut resp = [0u8; MSG_LEN];
        // qsid=7 (LE16)
        resp[0] = 0x07;
        resp[1] = 0x00;
        // qsid=21 (LE16)
        resp[2] = 0x15;
        resp[3] = 0x00;
        // sentinel
        resp[4] = 0xFF;
        resp[5] = 0xFF;

        let qsids = parse_qmk_settings_query(&resp);
        assert_eq!(qsids, vec![7, 21]);
    }

    #[test]
    fn matrix_state_parsing() {
        let mut resp = [0u8; MSG_LEN];
        resp[2] = 0b0000_0101; // row 0: cols 0,2 pressed
        resp[3] = 0b0000_0010; // row 1: col 1 pressed
        let m = parse_matrix_state(&resp, 2, 8);
        assert!(m[0][0]);
        assert!(!m[0][1]);
        assert!(m[0][2]);
        assert!(!m[1][0]);
        assert!(m[1][1]);
    }

    #[test]
    fn tap_dance_roundtrip() {
        let td = TapDanceEntry {
            on_tap: 0x0004,
            on_hold: 0x00E1,
            on_double_tap: 0x0004,
            on_tap_hold: 0x0000,
            tapping_term: 200,
        };
        let bytes = td.to_bytes();
        let td2 = TapDanceEntry::from_bytes(&bytes).unwrap();
        assert_eq!(td, td2);
    }
}
