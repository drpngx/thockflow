//! SVIL diagnostics commands (pointing, typing pause, debug console, flicker latch).
//!
//! Payload builders and reply parsers for the read-mostly diagnostics Keybard
//! polls: POINTING_HARDWARE (0x2C), POINTING_MOTION (0x2D, with its touch and
//! fingers pages), POINTING_CONTACTS (0x2F), POINTING_SENSOR (0x39),
//! TYPING_PAUSE (0x3C), FLICKER_LATCH (0x3D) and DEBUG_CONSOLE (0x3E). Wire
//! formats follow Keybard (`src/utils/pointing-*.ts`, `src/utils/touchpad-tuning.ts`,
//! `src/utils/velocity-scroll.ts`, `src/services/{typing-pause,flicker-latch,debug-console}.ts`)
//! and svalboard/qmk `keyboards/svalboard/docs/protocol.md`.
//!
//! As elsewhere in this module, payloads are `[cmd][args...]` (after the
//! wrapper's protocol byte) and replies start at the command echo.

use super::{le16, svil_payload, SvilCommand, SvilInfo};

// ---------------------------------------------------------------------------
// GET_INFO feature flags
// ---------------------------------------------------------------------------

/// `feature_flags2`: the board answers POINTING_HARDWARE (0x2C).
pub const FLAG2_POINTING_HARDWARE: u8 = 0x02;
/// `feature_flags2`: the board answers POINTING_MOTION (0x2D).
pub const FLAG2_POINTING_MOTION: u8 = 0x08;
/// `feature_flags2`: 0x2D keeps counts per reader id and reports each hand's CPI.
pub const FLAG2_POINTING_MOTION_READERS: u8 = 0x10;
/// `feature_flags2`: the board answers POINTING_CONTACTS (0x2F).
pub const FLAG2_POINTING_CONTACTS: u8 = 0x20;
/// `feature_flags3`: the board answers TYPING_PAUSE (0x3C).
pub const FLAG3_TYPING_PAUSE: u8 = 0x04;
/// `feature_flags3`: the board answers FLICKER_LATCH (0x3D).
pub const FLAG3_FLICKER_LATCH: u8 = 0x08;
/// `feature_flags3`: the touchpad tuning settings exist.
pub const FLAG3_PAD_TUNING: u8 = 0x20;
/// `feature_flags3`: the touch page of 0x2D (page 1).
pub const FLAG3_PAD_TOUCH: u8 = 0x40;
/// `feature_flags3`: the fingers page of 0x2D (page 2).
pub const FLAG3_PAD_FINGERS: u8 = 0x80;
/// `feature_flags4`: the touchpad palm rejection setting (VIA value 58).
pub const FLAG4_PAD_PALM: u8 = 0x01;
/// `feature_flags4`: the touchpad mode setting.
pub const FLAG4_PAD_MODE: u8 = 0x02;
/// `feature_flags4`: the board answers DEBUG_CONSOLE (0x3E).
pub const FLAG4_DEBUG_CONSOLE: u8 = 0x04;
/// `feature_flags4`: the board answers POINTING_SENSOR (0x39).
pub const FLAG4_POINTING_SENSOR: u8 = 0x08;

pub fn has_pointing_hardware(info: &SvilInfo) -> bool {
    info.feature_flags2 & FLAG2_POINTING_HARDWARE != 0
}
pub fn has_pointing_motion(info: &SvilInfo) -> bool {
    info.feature_flags2 & FLAG2_POINTING_MOTION != 0
}
pub fn has_pointing_motion_readers(info: &SvilInfo) -> bool {
    info.feature_flags2 & FLAG2_POINTING_MOTION_READERS != 0
}
pub fn has_pointing_contacts(info: &SvilInfo) -> bool {
    info.feature_flags2 & FLAG2_POINTING_CONTACTS != 0
}
pub fn has_typing_pause(info: &SvilInfo) -> bool {
    info.feature_flags3 & FLAG3_TYPING_PAUSE != 0
}
/// FLICKER_LATCH also needs a report of at least [`FLICKER_MIN_REPORT`] bytes.
pub fn has_flicker_latch(info: &SvilInfo, report_len: usize) -> bool {
    info.feature_flags3 & FLAG3_FLICKER_LATCH != 0 && report_len >= FLICKER_MIN_REPORT
}
pub fn has_pad_tuning(info: &SvilInfo) -> bool {
    info.feature_flags3 & FLAG3_PAD_TUNING != 0
}
pub fn has_pad_touch(info: &SvilInfo) -> bool {
    info.feature_flags3 & FLAG3_PAD_TOUCH != 0
}
pub fn has_pad_fingers(info: &SvilInfo) -> bool {
    info.feature_flags3 & FLAG3_PAD_FINGERS != 0
}
pub fn has_pad_palm(info: &SvilInfo) -> bool {
    info.feature_flags4 & FLAG4_PAD_PALM != 0
}
pub fn has_pad_mode(info: &SvilInfo) -> bool {
    info.feature_flags4 & FLAG4_PAD_MODE != 0
}
pub fn has_debug_console(info: &SvilInfo) -> bool {
    info.feature_flags4 & FLAG4_DEBUG_CONSOLE != 0
}
pub fn has_pointing_sensor(info: &SvilInfo) -> bool {
    info.feature_flags4 & FLAG4_POINTING_SENSOR != 0
}

/// Which hand / half.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

// ---------------------------------------------------------------------------
// POINTING_HARDWARE (0x2C)
// ---------------------------------------------------------------------------
//
// Reply: [0x2C][format = 1][USB half: 5][other half: 5]
// Each half: [flags][built_for][detected][id0][id1]

const HW_FLAG_REPORTED: u8 = 0x01;
const HW_FLAG_LEFT: u8 = 0x02;
const HW_FLAG_INIT_OK: u8 = 0x04;
const HW_FLAG_AUTO: u8 = 0x08;
const HW_FLAG_OS_TOUCHPAD: u8 = 0x10;
const HW_FLAG_NO_SIDE: u8 = 0x20;
const HW_BLOCK_SIZE: usize = 5;

/// A pointing module a half's firmware can drive or detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointingModule {
    Pmw3360,
    Pmw3389,
    Azoteq,
    TrackPoint,
}

impl PointingModule {
    /// Module code on the wire (1 PMW3360, 2 PMW3389, 5 Azoteq, 6 TrackPoint).
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Pmw3360),
            2 => Some(Self::Pmw3389),
            5 => Some(Self::Azoteq),
            6 => Some(Self::TrackPoint),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Pmw3360 => "PMW3360 trackball",
            Self::Pmw3389 => "PMW3389 trackball",
            Self::Azoteq => "Azoteq trackpad",
            Self::TrackPoint => "TrackPoint",
        }
    }
    pub fn is_trackball(self) -> bool {
        matches!(self, Self::Pmw3360 | Self::Pmw3389)
    }
}

/// What answered on a half's pointing bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedModule {
    Module(PointingModule),
    /// No module answered (code 3).
    None,
    /// The trackball bus answered with another ID (code 4).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointingHalf {
    /// The half plugged into the computer (it answers for both).
    pub usb: bool,
    pub side: Side,
    /// The module this half's firmware drives; `None` for a build without one.
    pub built_for: Option<PointingModule>,
    /// What answered; `None` when not checked yet.
    pub detected: Option<DetectedModule>,
    pub sensor_started: bool,
    /// The build drives whichever module answers at startup (`built_for` is the one it chose).
    pub auto: bool,
    /// The computer drives this half's touchpad as a touchpad (flags bit 4).
    pub os_touchpad: bool,
    /// One-image firmware: the half has no side yet. Such halves are dropped by
    /// [`parse_pointing_hardware`], as Keybard does.
    pub no_side: bool,
    pub product_id: u8,
    pub inverse_product_id: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointingHardware {
    /// The halves that reported: the USB half first, then the other half.
    pub halves: Vec<PointingHalf>,
}

/// `[0x2C]`.
pub fn pointing_hardware_payload() -> Vec<u8> {
    svil_payload(SvilCommand::PointingHardware, &[])
}

fn parse_hw_half(b: &[u8], off: usize, usb: bool) -> Option<PointingHalf> {
    if b.len() < off + HW_BLOCK_SIZE {
        return None;
    }
    let flags = b[off];
    if flags & HW_FLAG_REPORTED == 0 {
        return None;
    }
    let detected = match b[off + 2] {
        3 => Some(DetectedModule::None),
        4 => Some(DetectedModule::Unknown),
        c => PointingModule::from_code(c).map(DetectedModule::Module),
    };
    Some(PointingHalf {
        usb,
        side: if flags & HW_FLAG_LEFT != 0 { Side::Left } else { Side::Right },
        built_for: PointingModule::from_code(b[off + 1]),
        detected,
        sensor_started: flags & HW_FLAG_INIT_OK != 0,
        auto: flags & HW_FLAG_AUTO != 0,
        os_touchpad: flags & HW_FLAG_OS_TOUCHPAD != 0,
        no_side: flags & HW_FLAG_NO_SIDE != 0,
        product_id: b[off + 3],
        inverse_product_id: b[off + 4],
    })
}

/// Parse a POINTING_HARDWARE reply. `None` when malformed or nothing reported.
pub fn parse_pointing_hardware(reply: &[u8]) -> Option<PointingHardware> {
    if reply.len() < 2 || reply[0] != SvilCommand::PointingHardware as u8 || reply[1] != 1 {
        return None;
    }
    let halves: Vec<PointingHalf> = [parse_hw_half(reply, 2, true), parse_hw_half(reply, 2 + HW_BLOCK_SIZE, false)]
        .into_iter()
        .flatten()
        .filter(|h| !h.no_side)
        .collect();
    if halves.is_empty() {
        None
    } else {
        Some(PointingHardware { halves })
    }
}

// ---------------------------------------------------------------------------
// POINTING_MOTION (0x2D)
// ---------------------------------------------------------------------------

/// The reader id Keybard names in its 0x2D requests (Keybard Host is 2).
pub const KEYBARD_MOTION_READER: u8 = 1;
/// Format byte of the motion page. 0 means nothing to report.
pub const POINTING_MOTION_FORMAT: u8 = 1;
/// Payload `[53]` is `'F'` when the sniper / boost factors follow.
pub const POINTING_FACTORS_MARK: u8 = 0x46;
/// Marks the Velocity Scroll block.
pub const VELOCITY_SCROLL_MAGIC: u8 = 0x56;
/// The Velocity Scroll block's payload offset (format byte = 0) and length.
pub const VELOCITY_SCROLL_OFF: usize = 29;
pub const VELOCITY_SCROLL_LEN: usize = 24;

const MOTION_HALF_BYTES: usize = 10;
/// Command echo + 24 payload bytes.
const MOTION_REPLY_BYTES: usize = 2 + 24;
/// Command echo + the 24 bytes + reader id and each hand's CPI.
const MOTION_EXT_REPLY_BYTES: usize = 2 + 29;
const FACTORS_AT: usize = 53;

/// `[0x2D]`, or `[0x2D][reader]` on firmware with [`FLAG2_POINTING_MOTION_READERS`].
pub fn pointing_motion_payload(reader: Option<u8>) -> Vec<u8> {
    match reader {
        Some(r) => svil_payload(SvilCommand::PointingMotion, &[r]),
        None => svil_payload(SvilCommand::PointingMotion, &[]),
    }
}

/// The reader byte for a page request: [`KEYBARD_MOTION_READER`] with per-reader counts, else 0.
pub fn motion_reader(info: &SvilInfo) -> u8 {
    if has_pointing_motion_readers(info) {
        KEYBARD_MOTION_READER
    } else {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AxisLock {
    #[default]
    Free,
    Vertical,
    Horizontal,
}

impl AxisLock {
    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Vertical,
            2 => Self::Horizontal,
            _ => Self::Free,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MotionHalf {
    /// This hand has a started pointing driver whose data reaches the USB half.
    pub valid: bool,
    /// This hand's motion is being converted to scroll.
    pub scrolling: bool,
    /// dx or dy was clamped since the previous read.
    pub saturated: bool,
    pub auto_mouse: bool,
    pub touch_valid: bool,
    /// 0..3; 0 when unknown.
    pub fingers: u8,
    /// Raw sensor counts, +right / +down, before sniper and boost.
    pub dx: i16,
    pub dy: i16,
    pub h: i8,
    pub v: i8,
    pub buttons: u8,
    /// 0..255; only meaningful when `touch_valid`.
    pub touch_x: u8,
    pub touch_y: u8,
    /// The hand's sensor CPI, when the board reports it.
    pub cpi: Option<u16>,
}

/// One device's Velocity Scroll curve.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VelocityCurve {
    /// Dead zone per axis, mm.
    pub dead_mm: f32,
    /// Notches/s at 10 mm past the dead zone.
    pub gain: f32,
    pub exponent: f32,
    /// Speed cap, notches/s.
    pub max_nps: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VelocityScroll {
    /// Velocity Scroll is held now.
    pub held: bool,
    /// Which pad's finger sets the offset; `None` when the relative pointers do.
    pub pad_hand: Option<Side>,
    /// The axis lock tunable is on; `locked_axis` is the axis it picked (`Free` = none).
    pub axis_lock: bool,
    pub locked_axis: AxisLock,
    /// Offset from the anchor, mm: +x right, +y toward the user.
    pub offset_mm: (f32, f32),
    /// Speed the board sends at, notches/s: (h, v), +h right, +v up.
    pub speed_nps: (f32, f32),
    pub ball: VelocityCurve,
    pub pad: VelocityCurve,
}

/// The sniper divisor and boost multiplier in force (1 when that mode's key is off).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointingFactors {
    pub sniper_divisor: u8,
    pub boost_factor: u8,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PointingMotion {
    pub sniper: bool,
    pub boost: bool,
    pub auto_mouse_layer: bool,
    pub axis_lock_enabled: bool,
    pub natural_scroll: bool,
    pub scroll_key: bool,
    /// Milliseconds since the previous read (saturates at 255; 0 on the first read).
    pub elapsed_ms: u8,
    pub axis_lock: AxisLock,
    pub left: MotionHalf,
    pub right: MotionHalf,
    /// Present on firmware with a 64-byte reply carrying the block at payload 29-52.
    pub velocity_scroll: Option<VelocityScroll>,
    pub factors: Option<PointingFactors>,
}

fn parse_motion_half(b: &[u8], off: usize) -> MotionHalf {
    let flags = b[off];
    MotionHalf {
        valid: flags & 0x01 != 0,
        scrolling: flags & 0x02 != 0,
        saturated: flags & 0x04 != 0,
        auto_mouse: flags & 0x08 != 0,
        touch_valid: flags & 0x10 != 0,
        fingers: (flags >> 5) & 0x03,
        dx: le16(b, off + 1) as i16,
        dy: le16(b, off + 3) as i16,
        h: b[off + 5] as i8,
        v: b[off + 6] as i8,
        buttons: b[off + 7],
        touch_x: b[off + 8],
        touch_y: b[off + 9],
        cpi: None,
    }
}

/// Parse the Velocity Scroll block; `p` is the reply offset of the format byte.
fn parse_velocity_scroll(b: &[u8], p: usize) -> Option<VelocityScroll> {
    if b.len() < p + VELOCITY_SCROLL_OFF + VELOCITY_SCROLL_LEN {
        return None;
    }
    let o = p + VELOCITY_SCROLL_OFF;
    if b[o] != VELOCITY_SCROLL_MAGIC {
        return None;
    }
    let flags = b[o + 1];
    let s16 = |k: usize| le16(b, o + k) as i16 as f32;
    let u16f = |k: usize| le16(b, o + k) as f32;
    let max_nps = u16f(10) / 100.0;
    Some(VelocityScroll {
        held: flags & 0x01 != 0,
        pad_hand: if flags & 0x02 != 0 {
            Some(if flags & 0x04 != 0 { Side::Right } else { Side::Left })
        } else {
            None
        },
        axis_lock: flags & 0x08 != 0,
        locked_axis: AxisLock::from_code((flags >> 4) & 3),
        offset_mm: (s16(2) / 100.0, s16(4) / 100.0),
        speed_nps: (s16(6) / 100.0, s16(8) / 100.0),
        ball: VelocityCurve { dead_mm: u16f(12) / 100.0, gain: u16f(14) / 100.0, exponent: u16f(16) / 1000.0, max_nps },
        pad: VelocityCurve { dead_mm: u16f(18) / 100.0, gain: u16f(20) / 100.0, exponent: u16f(22) / 1000.0, max_nps },
    })
}

/// Parse a POINTING_MOTION reply (motion page). `None` unless the echo
/// matches, the format is 1 and the reply is complete.
pub fn parse_pointing_motion(reply: &[u8]) -> Option<PointingMotion> {
    if reply.len() < MOTION_REPLY_BYTES
        || reply[0] != SvilCommand::PointingMotion as u8
        || reply[1] != POINTING_MOTION_FORMAT
    {
        return None;
    }
    let p = 1;
    let mode = reply[p + 1];
    let mut left = parse_motion_half(reply, p + 3);
    let mut right = parse_motion_half(reply, p + 3 + MOTION_HALF_BYTES);
    if reply.len() >= MOTION_EXT_REPLY_BYTES {
        left.cpi = Some(le16(reply, p + 25)).filter(|&c| c > 0);
        right.cpi = Some(le16(reply, p + 27)).filter(|&c| c > 0);
    }
    let factors = if reply.len() > p + FACTORS_AT + 2 && reply[p + FACTORS_AT] == POINTING_FACTORS_MARK {
        Some(PointingFactors {
            sniper_divisor: reply[p + FACTORS_AT + 1].max(1),
            boost_factor: reply[p + FACTORS_AT + 2].max(1),
        })
    } else {
        None
    };
    Some(PointingMotion {
        sniper: mode & 0x01 != 0,
        boost: mode & 0x02 != 0,
        auto_mouse_layer: mode & 0x04 != 0,
        axis_lock_enabled: mode & 0x08 != 0,
        natural_scroll: mode & 0x10 != 0,
        scroll_key: mode & 0x20 != 0,
        elapsed_ms: reply[p + 2],
        axis_lock: AxisLock::from_code(reply[p + 23]),
        left,
        right,
        velocity_scroll: parse_velocity_scroll(reply, p),
        factors,
    })
}

// --- Touch page (0x2D page 1) ----------------------------------------------

pub const PAD_TOUCH_PAGE: u8 = 1;
pub const PAD_TOUCH_FORMAT: u8 = 2;
/// Pad size used when the board reports 0, 0.01 mm.
pub const PAD_WIDTH_MM100: u16 = 4300;
pub const PAD_HEIGHT_MM100: u16 = 4000;
const PAD_TOUCH_PAGE_BYTES: usize = 48;
/// Firmware with finger sizes sends 56 (bytes 48-55).
const PAD_TOUCH_SIZED_PAGE_BYTES: usize = 56;

/// Touch page `events` bits.
pub const TOUCH_EV_DOWN: u8 = 0x01;
pub const TOUCH_EV_UP: u8 = 0x02;
pub const TOUCH_EV_TAP: u8 = 0x04;
pub const TOUCH_EV_TWO_TAP: u8 = 0x08;
pub const TOUCH_EV_HOLD: u8 = 0x10;
pub const TOUCH_EV_SCROLL: u8 = 0x20;
pub const TOUCH_EV_DRIFT: u8 = 0x40;
pub const TOUCH_EV_OTHER: u8 = 0x80;

/// `[0x2D][reader][page]` (pages [`PAD_TOUCH_PAGE`], [`PAD_FINGERS_PAGE`]).
pub fn pointing_motion_page_payload(reader: u8, page: u8) -> Vec<u8> {
    svil_payload(SvilCommand::PointingMotion, &[reader, page])
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PadTouchBlock {
    pub valid: bool,
    pub down: bool,
    /// The computer has this pad as a touchpad.
    pub os: bool,
    pub outside: bool,
    pub fingers: u8,
    pub holding: bool,
    /// `TOUCH_EV_*` bits since the last read.
    pub events: u8,
    /// 0.01 mm, in the pad's own orientation.
    pub x: u16,
    pub y: u16,
    pub down_x: u16,
    pub down_y: u16,
    /// ms since touch-down while down; the last touch's length after it lifts.
    pub ms: u16,
    /// The farthest this touch has been from its down point, 0.01 mm.
    pub farthest: u16,
    pub tap_px: u16,
    /// First finger's touch strength and area (channels); 0 when not reported.
    pub strength: u16,
    pub area: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PadTouch {
    pub tap_on: bool,
    pub two_finger_on: bool,
    pub hold_on: bool,
    pub elapsed_ms: u8,
    pub dead_zone: u16,
    pub tap_ms: u16,
    pub hold_ms: u16,
    pub width: u16,
    pub height: u16,
    pub scroll_start: u16,
    /// The channel grid the area counts in (0 when no finger size is reported).
    pub channels_x: u8,
    pub channels_y: u8,
    pub left: PadTouchBlock,
    pub right: PadTouchBlock,
}

/// `b` starts at the format byte.
fn parse_touch_block(b: &[u8], o: usize) -> PadTouchBlock {
    let f = b[o];
    let u = |k: usize| le16(b, o + k);
    PadTouchBlock {
        valid: f & 0x01 != 0,
        down: f & 0x02 != 0,
        os: f & 0x04 != 0,
        outside: f & 0x08 != 0,
        fingers: (f >> 4) & 7,
        holding: f & 0x80 != 0,
        events: b[o + 1],
        x: u(2),
        y: u(4),
        down_x: u(6),
        down_y: u(8),
        ms: u(10),
        farthest: u(12),
        tap_px: u(14),
        strength: 0,
        area: 0,
    }
}

/// Parse the reply to `[0x2D][reader][1]`. `None` for anything but format 2.
pub fn parse_pad_touch(reply: &[u8]) -> Option<PadTouch> {
    if reply.len() < 1 + PAD_TOUCH_PAGE_BYTES
        || reply[0] != SvilCommand::PointingMotion as u8
        || reply[1] != PAD_TOUCH_FORMAT
    {
        return None;
    }
    let sized = reply.len() >= 1 + PAD_TOUCH_SIZED_PAGE_BYTES && reply[1 + 3] & 1 != 0;
    let b = &reply[1..1 + if sized { PAD_TOUCH_SIZED_PAGE_BYTES } else { PAD_TOUCH_PAGE_BYTES }];
    let u = |k: usize| le16(b, k);
    let on = b[1];
    let mut left = parse_touch_block(b, 14);
    let mut right = parse_touch_block(b, 30);
    if sized {
        if left.valid {
            left.strength = u(48);
            left.area = b[50];
        }
        if right.valid {
            right.strength = u(51);
            right.area = b[53];
        }
    }
    let or = |v: u16, d: u16| if v == 0 { d } else { v };
    Some(PadTouch {
        tap_on: on & 1 != 0,
        two_finger_on: on & 2 != 0,
        hold_on: on & 4 != 0,
        elapsed_ms: b[2],
        dead_zone: u(4),
        tap_ms: u(6),
        hold_ms: u(8),
        width: or(u(10), PAD_WIDTH_MM100),
        height: or(u(12), PAD_HEIGHT_MM100),
        scroll_start: u(46),
        channels_x: if sized { b[54] } else { 0 },
        channels_y: if sized { b[55] } else { 0 },
        left,
        right,
    })
}

// --- Fingers page (0x2D page 2) --------------------------------------------

pub const PAD_FINGERS_PAGE: u8 = 2;
pub const PAD_FINGERS_FORMAT: u8 = 1;
const FINGERS_PAGE_BYTES: usize = 56;
const FINGERS_BLOCK_BYTES: usize = 27;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadFinger {
    /// The chip's slot: a finger keeps it while it is down.
    pub slot: u8,
    /// 0.01 mm, in the pad's own orientation (0.02 mm steps).
    pub x: u16,
    pub y: u16,
    /// Channels the finger covers.
    pub area: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PadFingersBlock {
    pub valid: bool,
    /// The computer has this pad as a touchpad.
    pub os: bool,
    /// The chip's own palm detect.
    pub chip_palm: bool,
    /// More fingers than the chip reports.
    pub too_many: bool,
    /// A two-finger touch is live.
    pub two: bool,
    /// The chip started a scroll during that two-finger touch.
    pub scrolled: bool,
    /// The chip reported a two-finger tap since the last read.
    pub two_tap: bool,
    /// The two-finger numbers below hold a touch (the live one, or the last).
    pub had_two: bool,
    pub fingers: Vec<PadFinger>,
    /// The two fingers' centroid when the second landed, 0.01 mm.
    pub start_x: u16,
    pub start_y: u16,
    /// The two fingers' average travel since then, 0.01 mm.
    pub travel: u16,
    /// ms since the second finger landed (or how long the two-finger touch lasted).
    pub two_ms: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PadFingers {
    /// The palm rejection threshold in force, channels; 0 off.
    pub palm: u8,
    pub left: PadFingersBlock,
    pub right: PadFingersBlock,
}

/// A packed 12+12-bit position, in 0.02 mm units, as 0.01 mm.
fn finger_pos(b: &[u8], o: usize) -> (u16, u16) {
    let v = b[o] as u32 | (b[o + 1] as u32) << 8 | (b[o + 2] as u32) << 16;
    (((v & 0xfff) * 2) as u16, (((v >> 12) & 0xfff) * 2) as u16)
}

fn parse_fingers_block(b: &[u8], o: usize) -> PadFingersBlock {
    let f = b[o];
    if f & 0x01 == 0 {
        return PadFingersBlock::default();
    }
    let mask = b[o + 1];
    let fingers = (0..5u8)
        .filter(|slot| mask & (1 << slot) != 0)
        .map(|slot| {
            let at = o + 2 + 4 * slot as usize;
            let (x, y) = finger_pos(b, at);
            PadFinger { slot, x, y, area: b[at + 3] }
        })
        .collect();
    let (start_x, start_y) = finger_pos(b, o + 22);
    PadFingersBlock {
        valid: true,
        os: f & 0x02 != 0,
        chip_palm: f & 0x04 != 0,
        too_many: f & 0x08 != 0,
        two: f & 0x10 != 0,
        scrolled: f & 0x20 != 0,
        two_tap: f & 0x40 != 0,
        had_two: f & 0x80 != 0,
        fingers,
        start_x,
        start_y,
        travel: b[o + 25] as u16 * 5,
        two_ms: b[o + 26] as u16 * 4,
    }
}

/// Parse the reply to `[0x2D][reader][2]`. `None` for anything but format 1.
pub fn parse_pad_fingers(reply: &[u8]) -> Option<PadFingers> {
    if reply.len() < 1 + FINGERS_PAGE_BYTES
        || reply[0] != SvilCommand::PointingMotion as u8
        || reply[1] != PAD_FINGERS_FORMAT
    {
        return None;
    }
    let b = &reply[1..1 + FINGERS_PAGE_BYTES];
    Some(PadFingers {
        palm: b[1],
        left: parse_fingers_block(b, 2),
        right: parse_fingers_block(b, 2 + FINGERS_BLOCK_BYTES),
    })
}

// ---------------------------------------------------------------------------
// POINTING_CONTACTS (0x2F)
// ---------------------------------------------------------------------------
//
// Reply: [0x2F][format = 1][flags][age][count][contact: 4 bytes]...
// Contact: [b][xy: 3 bytes, x low 12 bits, y high 12 bits]
//   b: bits 0-3 id, bit 4 tip, bit 5 confident, bit 6 right pad, bit 7 left out

pub const POINTING_CONTACTS_FORMAT: u8 = 1;
/// Counts across one pad (x) and down it (y).
pub const PAD_COUNTS_X: u16 = 2048;
pub const PAD_COUNTS_Y: u16 = 1792;
/// The most contacts a reply carries.
pub const MAX_CONTACTS: usize = 6;
const CONTACTS_HEADER_BYTES: usize = 1 + 4;
const CONTACT_BYTES: usize = 4;

/// `[0x2F]`.
pub fn pointing_contacts_payload() -> Vec<u8> {
    svil_payload(SvilCommand::PointingContacts, &[])
}

/// A pad count to -1..1: 0 is one edge, `counts - 1` the other.
pub fn pad_unit(count: u16, counts: u16) -> f32 {
    let max = counts.saturating_sub(1);
    let c = count.min(max);
    if max == 0 {
        return -1.0;
    }
    (c as f32 / max as f32) * 2.0 - 1.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadContact {
    pub id: u8,
    /// Raw pad counts (x across, y down).
    pub raw_x: u16,
    pub raw_y: u16,
    /// -1..1 from the pad's left / top edge to its right / bottom one.
    pub x: f32,
    pub y: f32,
    /// Clear when the touchpad has flagged it as a palm.
    pub confident: bool,
    /// A finger the touchpad left out; its ID is from the board's own space.
    pub left_out: bool,
}

impl PadContact {
    /// One key per finger: a left-out contact's ID space is its own.
    pub fn key(&self) -> u8 {
        if self.left_out {
            16 + self.id
        } else {
            self.id
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PointingContacts {
    /// The touchpad is reporting to the computer.
    pub reporting: bool,
    /// Which pads are part of the touchpad.
    pub pad_left: bool,
    pub pad_right: bool,
    /// Fingers down on each pad (lifting contacts are left out).
    pub left: Vec<PadContact>,
    pub right: Vec<PadContact>,
}

/// Parse a POINTING_CONTACTS reply. `None` unless the echo matches, the
/// format is 1 and the reply holds every contact its count names.
pub fn parse_pointing_contacts(reply: &[u8]) -> Option<PointingContacts> {
    if reply.len() < CONTACTS_HEADER_BYTES
        || reply[0] != SvilCommand::PointingContacts as u8
        || reply[1] != POINTING_CONTACTS_FORMAT
    {
        return None;
    }
    let flags = reply[2];
    let n = (reply[4] as usize).min(MAX_CONTACTS);
    if reply.len() < CONTACTS_HEADER_BYTES + CONTACT_BYTES * n {
        return None;
    }
    let mut out = PointingContacts {
        reporting: flags & 0x01 != 0,
        pad_left: flags & 0x02 != 0,
        pad_right: flags & 0x04 != 0,
        left: Vec::new(),
        right: Vec::new(),
    };
    for i in 0..n {
        let o = CONTACTS_HEADER_BYTES + CONTACT_BYTES * i;
        let b = reply[o];
        if b & 0x10 == 0 {
            continue; // tip off: lifting
        }
        let w = reply[o + 1] as u32 | (reply[o + 2] as u32) << 8 | (reply[o + 3] as u32) << 16;
        let raw_x = (w & 0xfff) as u16;
        let raw_y = ((w >> 12) & 0xfff) as u16;
        let contact = PadContact {
            id: b & 0x0f,
            raw_x,
            raw_y,
            x: pad_unit(raw_x, PAD_COUNTS_X),
            y: pad_unit(raw_y, PAD_COUNTS_Y),
            confident: b & 0x20 != 0,
            left_out: b & 0x80 != 0,
        };
        if b & 0x40 != 0 {
            out.right.push(contact);
        } else {
            out.left.push(contact);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// POINTING_SENSOR (0x39)
// ---------------------------------------------------------------------------
//
// Request: [0x39][op][hand][cursor:2 LE]
// SAMPLES reply: [0x39][format = 1][flags][first_seq:2][count][head:2][bin: 9 bytes]...
//   flags: bit 0 present, 1 PMW3389, 2 lost, 3 more, 4 right, bits 5-6 op

pub const SENSOR_OP_SAMPLES: u8 = 0;
pub const SENSOR_OP_STATS: u8 = 1;
/// Each bin is a 50 ms fold of the half's sensor reads.
pub const SENSOR_BIN_MS: u32 = 50;
pub const SENSOR_MAX_BINS_PER_REPLY: usize = 2;
const SENSOR_REPLY_BYTES: usize = 1 + 25;
const SENSOR_BIN_BYTES: usize = 9;

/// `[0x39][0][hand][cursor:2 LE]`; hand 0 left, 1 right.
pub fn pointing_sensor_samples_payload(hand: u8, cursor: u16) -> Vec<u8> {
    let c = cursor.to_le_bytes();
    svil_payload(SvilCommand::PointingSensor, &[SENSOR_OP_SAMPLES, hand, c[0], c[1]])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorBin {
    /// 16-bit sequence number; counts on and wraps.
    pub seq: u16,
    pub squal_mean: u8,
    pub squal_min: u8,
    pub squal_max: u8,
    pub raw_sum: u8,
    pub raw_max: u8,
    pub raw_min: u8,
    pub shutter: u16,
    /// Reads with motion in the bin (0 to 127).
    pub moving: u8,
    /// The ball was lifted at some read of the bin.
    pub lifted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorSamples {
    /// A PMW sensor runs on that hand.
    pub present: bool,
    pub is_3389: bool,
    /// The cursor was older than the ring or ahead of it; the reply starts at the oldest bin.
    pub lost: bool,
    pub more: bool,
    pub right: bool,
    /// Sequence number of the next bin to be made.
    pub head: u16,
    pub bins: Vec<SensorBin>,
}

/// Distance from `a` to `b` on the 16-bit sequence (positive when `b` is newer).
pub fn seq_diff(b: u16, a: u16) -> i16 {
    b.wrapping_sub(a) as i16
}

/// Parse a SAMPLES reply. `None` unless the echo matches, the format is 1,
/// the op is SAMPLES and the reply is complete.
pub fn parse_sensor_samples(reply: &[u8]) -> Option<SensorSamples> {
    if reply.len() < SENSOR_REPLY_BYTES || reply[0] != SvilCommand::PointingSensor as u8 || reply[1] != 1 {
        return None;
    }
    let p = 1;
    let flags = reply[p + 1];
    if (flags >> 5) & 3 != SENSOR_OP_SAMPLES {
        return None;
    }
    let first_seq = le16(reply, p + 2);
    let count = (reply[p + 4] as usize).min(SENSOR_MAX_BINS_PER_REPLY);
    let head = le16(reply, p + 5);
    let mut bins = Vec::with_capacity(count);
    for i in 0..count {
        let o = p + 7 + i * SENSOR_BIN_BYTES;
        if o + SENSOR_BIN_BYTES > reply.len() {
            break;
        }
        bins.push(SensorBin {
            seq: first_seq.wrapping_add(i as u16),
            squal_mean: reply[o],
            squal_min: reply[o + 1],
            squal_max: reply[o + 2],
            raw_sum: reply[o + 3],
            raw_max: reply[o + 4],
            raw_min: reply[o + 5],
            shutter: le16(reply, o + 6),
            moving: reply[o + 8] & 0x7f,
            lifted: reply[o + 8] & 0x80 != 0,
        });
    }
    Some(SensorSamples {
        present: flags & 0x01 != 0,
        is_3389: flags & 0x02 != 0,
        lost: flags & 0x04 != 0,
        more: flags & 0x08 != 0,
        right: flags & 0x10 != 0,
        head,
        bins,
    })
}

// ---------------------------------------------------------------------------
// TYPING_PAUSE (0x3C)
// ---------------------------------------------------------------------------
//
//   [0x3C][op]  op 0 off, 1 on (also renews; the sender owns the pause), 2 status
//   -> [0x3C][op][status][state][last end][flags][lease ms:2][keyboard blocked:2]
//      [extra blocked:2][mouse passed:2][keys released][empty reports][keys ignored]

pub const TYPING_PAUSE_OFF: u8 = 0;
pub const TYPING_PAUSE_ON: u8 = 1;
pub const TYPING_PAUSE_STATUS: u8 = 2;
/// How often Keybard renews the pause; the board's lease is 1.5 s.
pub const TYPING_PAUSE_RENEW_MS: u32 = 500;

/// `[0x3C][op]`.
pub fn typing_pause_payload(op: u8) -> Vec<u8> {
    svil_payload(SvilCommand::TypingPause, &[op])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypingPauseState {
    #[default]
    Off,
    Paused,
    Ending,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypingPauseReply {
    /// Status 0.
    pub ok: bool,
    pub state: TypingPauseState,
    /// The pause is on and this client owns it.
    pub owned: bool,
    pub lease_ms: u16,
    pub keyboard_blocked: u16,
    pub extra_blocked: u16,
    pub mouse_passed: u16,
    pub keys_released: u8,
    pub empty_reports: u8,
    pub keys_ignored: u8,
}

pub fn parse_typing_pause(reply: &[u8]) -> Option<TypingPauseReply> {
    if reply.len() < 17 || reply[0] != SvilCommand::TypingPause as u8 {
        return None;
    }
    let u = |o: usize| le16(reply, o);
    Some(TypingPauseReply {
        ok: reply[2] == 0,
        state: match reply[3] {
            1 => TypingPauseState::Paused,
            2 => TypingPauseState::Ending,
            _ => TypingPauseState::Off,
        },
        owned: reply[5] & 1 != 0,
        lease_ms: u(6),
        keyboard_blocked: u(8),
        extra_blocked: u(10),
        mouse_passed: u(12),
        keys_released: reply[14],
        empty_reports: reply[15],
        keys_ignored: reply[16],
    })
}

// ---------------------------------------------------------------------------
// FLICKER_LATCH (0x3D)
// ---------------------------------------------------------------------------
//
//   [0x3D][0]        -> [0x3D][0][status][flags][rows][raw:rows][pressed:rows][released:rows]
//   [0x3D][1][half]  -> [0x3D][1][status][valid][half][changes: half_rows x cols]
//
// Reads clear what they return. One byte per row, bit = column.

/// FLICKER_LATCH command byte (not in [`SvilCommand`]).
pub const CMD_FLICKER_LATCH: u8 = 0x3D;
pub const FLICKER_READ: u8 = 0;
pub const FLICKER_COUNTS: u8 = 1;
/// The reply needs a 64-byte report.
pub const FLICKER_MIN_REPORT: usize = 64;

/// `[0x3D][0]`.
pub fn flicker_read_payload() -> Vec<u8> {
    vec![CMD_FLICKER_LATCH, FLICKER_READ]
}

/// `[0x3D][1][half]`; half 0 left, 1 right.
pub fn flicker_counts_payload(half: u8) -> Vec<u8> {
    vec![CMD_FLICKER_LATCH, FLICKER_COUNTS, half]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlickerRead {
    /// Which row blocks came back: bit 0 left (rows 0-4), bit 1 right (5-9).
    pub halves: u8,
    pub raw: Vec<u8>,
    pub pressed: Vec<u8>,
    pub released: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlickerCounts {
    pub half: u8,
    /// Changes per switch, rows of `cols`, this half's rows only.
    pub changes: Vec<u8>,
}

pub fn parse_flicker_read(reply: &[u8]) -> Option<FlickerRead> {
    if reply.len() < 5 || reply[0] != CMD_FLICKER_LATCH || reply[1] != FLICKER_READ || reply[2] != 0 {
        return None;
    }
    let rows = reply[4] as usize;
    if rows == 0 || reply.len() < 5 + 3 * rows {
        return None;
    }
    Some(FlickerRead {
        halves: reply[3],
        raw: reply[5..5 + rows].to_vec(),
        pressed: reply[5 + rows..5 + 2 * rows].to_vec(),
        released: reply[5 + 2 * rows..5 + 3 * rows].to_vec(),
    })
}

/// `half_rows` is the matrix rows / 2, `cols` the matrix columns.
pub fn parse_flicker_counts(reply: &[u8], half_rows: usize, cols: usize) -> Option<FlickerCounts> {
    if reply.len() < 5
        || reply[0] != CMD_FLICKER_LATCH
        || reply[1] != FLICKER_COUNTS
        || reply[2] != 0
        || reply[3] & 1 == 0
    {
        return None;
    }
    let n = half_rows * cols;
    if reply.len() < 5 + n {
        return None;
    }
    Some(FlickerCounts { half: reply[4], changes: reply[5..5 + n].to_vec() })
}

/// A switch's presses since the last read, from its changes and whether it was
/// down at that read: half the changes, the odd one a press if it was up.
pub fn presses_from(changes: u32, was_down: bool) -> u32 {
    if was_down {
        changes / 2
    } else {
        changes.div_ceil(2)
    }
}

// ---------------------------------------------------------------------------
// DEBUG_CONSOLE (0x3E)
// ---------------------------------------------------------------------------
//
//   [0x3E][op]  op 0 off, 1 on, 2 read
//   -> [0x3E][op][status][flag]   status 0 done, 1 unknown op, 2 no console, 3 too short

pub const DEBUG_CONSOLE_OFF: u8 = 0;
pub const DEBUG_CONSOLE_ON: u8 = 1;
pub const DEBUG_CONSOLE_READ: u8 = 2;

/// `[0x3E][op]`.
pub fn debug_console_payload(op: u8) -> Vec<u8> {
    svil_payload(SvilCommand::DebugConsole, &[op])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugConsoleReply {
    /// The board did what was asked (status 0).
    pub ok: bool,
    /// The flag after the request.
    pub on: bool,
}

/// `op` is the op that was sent; the reply must echo it.
pub fn parse_debug_console(reply: &[u8], op: u8) -> Option<DebugConsoleReply> {
    if reply.len() < 4 || reply[0] != SvilCommand::DebugConsole as u8 || reply[1] != op {
        return None;
    }
    Some(DebugConsoleReply { ok: reply[2] == 0, on: reply[3] == 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(cmd: u8, len: usize) -> Vec<u8> {
        let mut r = vec![0u8; len];
        r[0] = cmd;
        r
    }

    #[test]
    fn flags() {
        let info = SvilInfo {
            feature_flags2: FLAG2_POINTING_HARDWARE | FLAG2_POINTING_MOTION_READERS,
            feature_flags3: FLAG3_FLICKER_LATCH | FLAG3_PAD_TOUCH,
            feature_flags4: FLAG4_POINTING_SENSOR,
            ..Default::default()
        };
        assert!(has_pointing_hardware(&info));
        assert!(!has_pointing_motion(&info));
        assert_eq!(motion_reader(&info), KEYBARD_MOTION_READER);
        assert!(has_flicker_latch(&info, 64));
        assert!(!has_flicker_latch(&info, 32));
        assert!(has_pad_touch(&info) && !has_pad_fingers(&info));
        assert!(has_pointing_sensor(&info) && !has_debug_console(&info));
        assert_eq!(motion_reader(&SvilInfo::default()), 0);
    }

    #[test]
    fn pointing_hardware() {
        assert_eq!(pointing_hardware_payload(), vec![0x2C]);
        let mut r = reply(0x2C, 26);
        r[1] = 1;
        r[2..7].copy_from_slice(&[0x01 | 0x02 | 0x04, 1, 2, 0x47, 0xB8]);
        r[7..12].copy_from_slice(&[0x01 | 0x08 | 0x10, 5, 3, 0, 0]);
        let hw = parse_pointing_hardware(&r).unwrap();
        assert_eq!(hw.halves.len(), 2);
        let a = &hw.halves[0];
        assert!(a.usb && a.sensor_started && !a.auto);
        assert_eq!(a.side, Side::Left);
        assert_eq!(a.built_for, Some(PointingModule::Pmw3360));
        assert_eq!(a.detected, Some(DetectedModule::Module(PointingModule::Pmw3389)));
        assert_eq!((a.product_id, a.inverse_product_id), (0x47, 0xB8));
        let b = &hw.halves[1];
        assert!(!b.usb && b.auto && b.os_touchpad);
        assert_eq!(b.side, Side::Right);
        assert_eq!(b.built_for, Some(PointingModule::Azoteq));
        assert_eq!(b.detected, Some(DetectedModule::None));

        // Not checked yet / unknown / no side.
        r[4] = 0;
        r[7] = 0x01 | HW_FLAG_NO_SIDE;
        let hw = parse_pointing_hardware(&r).unwrap();
        assert_eq!(hw.halves.len(), 1);
        assert_eq!(hw.halves[0].detected, None);
        r[4] = 4;
        assert_eq!(parse_pointing_hardware(&r).unwrap().halves[0].detected, Some(DetectedModule::Unknown));

        // Nothing reported, wrong format, wrong echo.
        r[2] = 0;
        assert!(parse_pointing_hardware(&r).is_none());
        r[2] = 1;
        r[1] = 2;
        assert!(parse_pointing_hardware(&r).is_none());
        r[1] = 1;
        r[0] = 0x2D;
        assert!(parse_pointing_hardware(&r).is_none());
        assert!(parse_pointing_hardware(&[0x2C]).is_none());
    }

    #[test]
    fn pointing_motion_short() {
        assert_eq!(pointing_motion_payload(None), vec![0x2D]);
        assert_eq!(pointing_motion_payload(Some(1)), vec![0x2D, 1]);
        let mut r = reply(0x2D, 26);
        r[1] = 1;
        r[2] = 0x01 | 0x10 | 0x20;
        r[3] = 17;
        // Left half at 4.
        r[4] = 0x01 | 0x02 | 0x10 | (3 << 5);
        r[5..7].copy_from_slice(&(-5i16).to_le_bytes());
        r[7..9].copy_from_slice(&300i16.to_le_bytes());
        r[9] = 0xFF; // h = -1
        r[10] = 2;
        r[11] = 0x04;
        r[12] = 128;
        r[13] = 64;
        // Right half at 14.
        r[14] = 0x04 | 0x08;
        r[24] = 2;
        let m = parse_pointing_motion(&r).unwrap();
        assert!(m.sniper && !m.boost && m.natural_scroll && m.scroll_key);
        assert_eq!(m.elapsed_ms, 17);
        assert_eq!(m.axis_lock, AxisLock::Horizontal);
        assert!(m.left.valid && m.left.scrolling && m.left.touch_valid && !m.left.saturated);
        assert_eq!(m.left.fingers, 3);
        assert_eq!((m.left.dx, m.left.dy, m.left.h, m.left.v), (-5, 300, -1, 2));
        assert_eq!((m.left.buttons, m.left.touch_x, m.left.touch_y), (4, 128, 64));
        assert_eq!(m.left.cpi, None);
        assert!(m.right.saturated && m.right.auto_mouse && !m.right.valid);
        assert!(m.velocity_scroll.is_none() && m.factors.is_none());

        assert!(parse_pointing_motion(&r[..25]).is_none());
        r[1] = 0;
        assert!(parse_pointing_motion(&r).is_none());
    }

    #[test]
    fn pointing_motion_extended() {
        let mut r = reply(0x2D, 58);
        r[1] = 1;
        // CPI: payload 25/27 -> reply 26/28.
        r[26..28].copy_from_slice(&1600u16.to_le_bytes());
        // Velocity scroll block at reply 30.
        let o = 30;
        r[o] = VELOCITY_SCROLL_MAGIC;
        r[o + 1] = 0x01 | 0x02 | 0x04 | 0x08 | (1 << 4);
        r[o + 2..o + 4].copy_from_slice(&(-250i16).to_le_bytes());
        r[o + 4..o + 6].copy_from_slice(&100i16.to_le_bytes());
        r[o + 6..o + 8].copy_from_slice(&1200i16.to_le_bytes());
        r[o + 8..o + 10].copy_from_slice(&(-50i16).to_le_bytes());
        r[o + 10..o + 12].copy_from_slice(&6000u16.to_le_bytes());
        r[o + 12..o + 14].copy_from_slice(&80u16.to_le_bytes());
        r[o + 14..o + 16].copy_from_slice(&1200u16.to_le_bytes());
        r[o + 16..o + 18].copy_from_slice(&1500u16.to_le_bytes());
        r[o + 18..o + 20].copy_from_slice(&150u16.to_le_bytes());
        r[o + 20..o + 22].copy_from_slice(&1600u16.to_le_bytes());
        r[o + 22..o + 24].copy_from_slice(&2000u16.to_le_bytes());
        // Factors at payload 53 -> reply 54.
        r[54] = POINTING_FACTORS_MARK;
        r[55] = 3;
        r[56] = 0;
        let m = parse_pointing_motion(&r).unwrap();
        assert_eq!(m.left.cpi, Some(1600));
        assert_eq!(m.right.cpi, None);
        let vs = m.velocity_scroll.unwrap();
        assert!(vs.held && vs.axis_lock);
        assert_eq!(vs.pad_hand, Some(Side::Right));
        assert_eq!(vs.locked_axis, AxisLock::Vertical);
        assert_eq!(vs.offset_mm, (-2.5, 1.0));
        assert_eq!(vs.speed_nps, (12.0, -0.5));
        assert_eq!(vs.ball, VelocityCurve { dead_mm: 0.8, gain: 12.0, exponent: 1.5, max_nps: 60.0 });
        assert_eq!(vs.pad, VelocityCurve { dead_mm: 1.5, gain: 16.0, exponent: 2.0, max_nps: 60.0 });
        assert_eq!(m.factors, Some(PointingFactors { sniper_divisor: 3, boost_factor: 1 }));

        // Too short for the factors (needs > 56 bytes) and no magic.
        r[o] = 0;
        let m = parse_pointing_motion(&r[..56]).unwrap();
        assert!(m.factors.is_none() && m.velocity_scroll.is_none());
    }

    #[test]
    fn pad_touch() {
        assert_eq!(pointing_motion_page_payload(1, PAD_TOUCH_PAGE), vec![0x2D, 1, 1]);
        let mut r = reply(0x2D, 58);
        let b = 1; // reply offset of the format byte
        r[b] = PAD_TOUCH_FORMAT;
        r[b + 1] = 0x01 | 0x04;
        r[b + 2] = 30;
        r[b + 3] = 1; // sized
        r[b + 4..b + 6].copy_from_slice(&120u16.to_le_bytes());
        r[b + 6..b + 8].copy_from_slice(&150u16.to_le_bytes());
        r[b + 8..b + 10].copy_from_slice(&500u16.to_le_bytes());
        // width/height left 0 -> defaults
        r[b + 46..b + 48].copy_from_slice(&75u16.to_le_bytes());
        let l = b + 14;
        r[l] = 0x01 | 0x02 | (2 << 4) | 0x80;
        r[l + 1] = TOUCH_EV_DOWN | TOUCH_EV_TAP;
        r[l + 2..l + 4].copy_from_slice(&2150u16.to_le_bytes());
        r[l + 14..l + 16].copy_from_slice(&9u16.to_le_bytes());
        r[b + 48..b + 50].copy_from_slice(&700u16.to_le_bytes());
        r[b + 50] = 6;
        r[b + 51..b + 53].copy_from_slice(&99u16.to_le_bytes()); // right invalid: ignored
        r[b + 54] = 8;
        r[b + 55] = 7;
        let t = parse_pad_touch(&r).unwrap();
        assert!(t.tap_on && !t.two_finger_on && t.hold_on);
        assert_eq!((t.elapsed_ms, t.dead_zone, t.tap_ms, t.hold_ms), (30, 120, 150, 500));
        assert_eq!((t.width, t.height, t.scroll_start), (PAD_WIDTH_MM100, PAD_HEIGHT_MM100, 75));
        assert_eq!((t.channels_x, t.channels_y), (8, 7));
        assert!(t.left.valid && t.left.down && t.left.holding && !t.left.os);
        assert_eq!(t.left.fingers, 2);
        assert_eq!(t.left.events, TOUCH_EV_DOWN | TOUCH_EV_TAP);
        assert_eq!((t.left.x, t.left.tap_px, t.left.strength, t.left.area), (2150, 9, 700, 6));
        assert_eq!((t.right.strength, t.right.area), (0, 0));

        // Unsized (48-byte page) ignores bytes 48+.
        let t = parse_pad_touch(&r[..49]).unwrap();
        assert_eq!((t.channels_x, t.left.strength), (0, 0));
        assert!(parse_pad_touch(&r[..48]).is_none());
        r[b] = 1;
        assert!(parse_pad_touch(&r).is_none());
    }

    #[test]
    fn pad_fingers() {
        let mut r = reply(0x2D, 58);
        r[1] = PAD_FINGERS_FORMAT;
        r[2] = 12; // palm
        let o = 1 + 2; // left block, reply offset
        r[o] = 0x01 | 0x10 | 0x80;
        r[o + 1] = 0b00101; // slots 0 and 2
        // slot 0: x = 100 (raw 50), y = 200 (raw 100)
        let v: u32 = 50 | (100 << 12);
        r[o + 2..o + 5].copy_from_slice(&v.to_le_bytes()[..3]);
        r[o + 5] = 3;
        let v: u32 = 0xfff | (1 << 12);
        r[o + 10..o + 13].copy_from_slice(&v.to_le_bytes()[..3]);
        r[o + 13] = 9;
        let v: u32 = 10 | (20 << 12);
        r[o + 22..o + 25].copy_from_slice(&v.to_le_bytes()[..3]);
        r[o + 25] = 40;
        r[o + 26] = 255;
        // Right block invalid but with junk.
        r[o + 27 + 1] = 0xff;
        let f = parse_pad_fingers(&r).unwrap();
        assert_eq!(f.palm, 12);
        assert!(f.left.valid && f.left.two && f.left.had_two && !f.left.os);
        assert_eq!(
            f.left.fingers,
            vec![PadFinger { slot: 0, x: 100, y: 200, area: 3 }, PadFinger { slot: 2, x: 8190, y: 2, area: 9 }]
        );
        assert_eq!((f.left.start_x, f.left.start_y, f.left.travel, f.left.two_ms), (20, 40, 200, 1020));
        assert_eq!(f.right, PadFingersBlock::default());
        assert!(parse_pad_fingers(&r[..56]).is_none());
        r[1] = 2;
        assert!(parse_pad_fingers(&r).is_none());
    }

    #[test]
    fn pointing_contacts() {
        assert_eq!(pointing_contacts_payload(), vec![0x2F]);
        let mut r = reply(0x2F, 26);
        r[1] = 1;
        r[2] = 0x01 | 0x04;
        r[4] = 3;
        let put = |r: &mut Vec<u8>, i: usize, b: u8, x: u32, y: u32| {
            let o = 5 + 4 * i;
            r[o] = b;
            let w = x | (y << 12);
            r[o + 1..o + 4].copy_from_slice(&w.to_le_bytes()[..3]);
        };
        put(&mut r, 0, 0x10 | 0x20 | 3, 0, 1791);
        put(&mut r, 1, 0x10 | 0x40 | 0x80 | 1, 2047, 0);
        put(&mut r, 2, 0x20 | 2, 5, 5); // lifting
        let c = parse_pointing_contacts(&r).unwrap();
        assert!(c.reporting && !c.pad_left && c.pad_right);
        assert_eq!(c.left.len(), 1);
        assert_eq!(c.right.len(), 1);
        let a = c.left[0];
        assert_eq!((a.id, a.raw_x, a.raw_y, a.x, a.y), (3, 0, 1791, -1.0, 1.0));
        assert!(a.confident && !a.left_out);
        let b = c.right[0];
        assert_eq!((b.id, b.x, b.y, b.key()), (1, 1.0, -1.0, 17));
        assert!(!b.confident && b.left_out);

        assert!(parse_pointing_contacts(&r[..16]).is_none());
        assert!(parse_pointing_contacts(&r[..17]).is_some());
        r[1] = 0;
        assert!(parse_pointing_contacts(&r).is_none());
        assert_eq!(pad_unit(5000, PAD_COUNTS_X), 1.0);
    }

    #[test]
    fn pointing_sensor() {
        assert_eq!(pointing_sensor_samples_payload(1, 0x1234), vec![0x39, 0, 1, 0x34, 0x12]);
        let mut r = reply(0x39, 26);
        r[1] = 1;
        r[2] = 0x01 | 0x02 | 0x10;
        r[3..5].copy_from_slice(&0xFFFFu16.to_le_bytes());
        r[5] = 5; // clamped to 2
        r[6..8].copy_from_slice(&1u16.to_le_bytes());
        r[8..17].copy_from_slice(&[40, 30, 50, 7, 9, 2, 0x34, 0x12, 0x80 | 5]);
        r[17..26].copy_from_slice(&[41, 31, 51, 8, 10, 3, 0, 1, 127]);
        let s = parse_sensor_samples(&r).unwrap();
        assert!(s.present && s.is_3389 && s.right && !s.lost && !s.more);
        assert_eq!(s.head, 1);
        assert_eq!(s.bins.len(), 2);
        assert_eq!(
            s.bins[0],
            SensorBin {
                seq: 0xFFFF,
                squal_mean: 40,
                squal_min: 30,
                squal_max: 50,
                raw_sum: 7,
                raw_max: 9,
                raw_min: 2,
                shutter: 0x1234,
                moving: 5,
                lifted: true
            }
        );
        assert_eq!((s.bins[1].seq, s.bins[1].shutter, s.bins[1].moving, s.bins[1].lifted), (0, 256, 127, false));
        assert_eq!(seq_diff(0, 0xFFFF), 1);
        assert_eq!(seq_diff(0xFFFF, 0), -1);

        r[2] |= 1 << 5; // STATS op
        assert!(parse_sensor_samples(&r).is_none());
        r[2] &= !(3 << 5);
        assert!(parse_sensor_samples(&r[..25]).is_none());
        r[1] = 0;
        assert!(parse_sensor_samples(&r).is_none());
    }

    #[test]
    fn typing_pause() {
        assert_eq!(typing_pause_payload(TYPING_PAUSE_ON), vec![0x3C, 1]);
        let mut r = reply(0x3C, 26);
        r[1] = 1;
        r[3] = 1;
        r[5] = 1;
        r[6..8].copy_from_slice(&1500u16.to_le_bytes());
        r[8..10].copy_from_slice(&7u16.to_le_bytes());
        r[10..12].copy_from_slice(&8u16.to_le_bytes());
        r[12..14].copy_from_slice(&9u16.to_le_bytes());
        r[14] = 1;
        r[15] = 2;
        r[16] = 3;
        let t = parse_typing_pause(&r).unwrap();
        assert!(t.ok && t.owned);
        assert_eq!(t.state, TypingPauseState::Paused);
        assert_eq!((t.lease_ms, t.keyboard_blocked, t.extra_blocked, t.mouse_passed), (1500, 7, 8, 9));
        assert_eq!((t.keys_released, t.empty_reports, t.keys_ignored), (1, 2, 3));
        r[2] = 1;
        r[3] = 2;
        let t = parse_typing_pause(&r).unwrap();
        assert!(!t.ok);
        assert_eq!(t.state, TypingPauseState::Ending);
        assert!(parse_typing_pause(&r[..16]).is_none());
    }

    #[test]
    fn flicker_latch() {
        assert_eq!(flicker_read_payload(), vec![0x3D, 0]);
        assert_eq!(flicker_counts_payload(1), vec![0x3D, 1, 1]);
        let mut r = reply(0x3D, 58);
        r[3] = 0b11;
        r[4] = 2;
        r[5..11].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        let f = parse_flicker_read(&r).unwrap();
        assert_eq!(f.halves, 3);
        assert_eq!((f.raw, f.pressed, f.released), (vec![1, 2], vec![3, 4], vec![5, 6]));
        assert!(parse_flicker_read(&r[..10]).is_none());
        r[4] = 0;
        assert!(parse_flicker_read(&r).is_none());
        r[4] = 2;
        r[2] = 1;
        assert!(parse_flicker_read(&r).is_none());

        let mut r = reply(0x3D, 58);
        r[1] = FLICKER_COUNTS;
        r[3] = 1;
        r[4] = 1;
        for i in 0..30 {
            r[5 + i] = i as u8;
        }
        let c = parse_flicker_counts(&r, 5, 6).unwrap();
        assert_eq!(c.half, 1);
        assert_eq!(c.changes, (0..30).collect::<Vec<u8>>());
        assert!(parse_flicker_counts(&r[..34], 5, 6).is_none());
        r[3] = 0;
        assert!(parse_flicker_counts(&r, 5, 6).is_none());

        assert_eq!(presses_from(3, false), 2);
        assert_eq!(presses_from(3, true), 1);
        assert_eq!(presses_from(4, false), 2);
    }

    #[test]
    fn debug_console() {
        assert_eq!(debug_console_payload(DEBUG_CONSOLE_READ), vec![0x3E, 2]);
        let r = [0x3E, 2, 0, 1];
        assert_eq!(parse_debug_console(&r, 2), Some(DebugConsoleReply { ok: true, on: true }));
        assert_eq!(parse_debug_console(&[0x3E, 1, 2, 0], 1), Some(DebugConsoleReply { ok: false, on: false }));
        assert!(parse_debug_console(&r, 1).is_none());
        assert!(parse_debug_console(&r[..3], 2).is_none());
    }
}
