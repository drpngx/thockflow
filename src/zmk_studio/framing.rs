//! Byte-stuffing framing protocol for ZMK Studio RPC.
//!
//! Frames are delimited by SOF/EOF markers. Special bytes within the payload
//! are escaped with an ESCAPE byte followed by the original byte XOR'd with 0x20.

const SOF: u8 = 0xAB;
const ESCAPE: u8 = 0xAC;
const EOF_MARKER: u8 = 0xAD;

fn is_special(b: u8) -> bool {
    b == SOF || b == ESCAPE || b == EOF_MARKER
}

/// Encode a payload into a framed byte sequence.
pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.push(SOF);
    for &b in payload {
        if is_special(b) {
            out.push(ESCAPE);
        }
        out.push(b);
    }
    out.push(EOF_MARKER);
    out
}

/// State machine that accumulates incoming bytes and yields complete frames.
#[derive(Default)]
pub struct FrameDecoder {
    state: DecoderState,
    buf: Vec<u8>,
}

#[derive(Default, PartialEq)]
enum DecoderState {
    #[default]
    Idle,
    AwaitingData,
    Escaped,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw bytes from the transport. Returns any complete frames decoded.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        for &b in data {
            match self.state {
                DecoderState::Idle => {
                    if b == SOF {
                        self.buf.clear();
                        self.state = DecoderState::AwaitingData;
                    }
                    // Discard bytes outside a frame.
                }
                DecoderState::AwaitingData => {
                    if b == EOF_MARKER {
                        frames.push(std::mem::take(&mut self.buf));
                        self.state = DecoderState::Idle;
                    } else if b == ESCAPE {
                        self.state = DecoderState::Escaped;
                    } else if b == SOF {
                        // New frame start — discard current partial frame.
                        self.buf.clear();
                    } else {
                        self.buf.push(b);
                    }
                }
                DecoderState::Escaped => {
                    self.buf.push(b);
                    self.state = DecoderState::AwaitingData;
                }
            }
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_simple() {
        let payload = b"hello";
        let frame = encode_frame(payload);
        let mut dec = FrameDecoder::new();
        let frames = dec.feed(&frame);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], payload);
    }

    #[test]
    fn round_trip_special_bytes() {
        let payload = vec![SOF, ESCAPE, EOF_MARKER, 0x42];
        let frame = encode_frame(&payload);
        // Expected frame: SOF, ESCAPE, SOF, ESCAPE, ESCAPE, ESCAPE, EOF_MARKER, 0x42, EOF_MARKER
        // (Since all special bytes are escaped)
        let mut dec = FrameDecoder::new();
        let frames = dec.feed(&frame);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], payload);
    }

    #[test]
    fn partial_frames() {
        let payload = b"test";
        let frame = encode_frame(payload);
        let mid = frame.len() / 2;

        let mut dec = FrameDecoder::new();
        let frames = dec.feed(&frame[..mid]);
        assert!(frames.is_empty());
        let frames = dec.feed(&frame[mid..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], payload);
    }

    #[test]
    fn multiple_frames_in_one_feed() {
        let p1 = b"one";
        let p2 = b"two";
        let mut data = encode_frame(p1);
        data.extend_from_slice(&encode_frame(p2));

        let mut dec = FrameDecoder::new();
        let frames = dec.feed(&data);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], p1);
        assert_eq!(frames[1], p2);
    }

    #[test]
    fn garbage_before_sof_is_discarded() {
        let payload = b"ok";
        let mut data = vec![0x00, 0x01, 0xFF];
        data.extend_from_slice(&encode_frame(payload));

        let mut dec = FrameDecoder::new();
        let frames = dec.feed(&data);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], payload);
    }

    #[test]
    fn empty_payload() {
        let frame = encode_frame(&[]);
        let mut dec = FrameDecoder::new();
        let frames = dec.feed(&frame);
        assert_eq!(frames.len(), 1);
        assert!(frames[0].is_empty());
    }
}
