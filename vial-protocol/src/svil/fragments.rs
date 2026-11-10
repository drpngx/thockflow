//! SVIL fragment commands (modular layout hardware detection and selections).
//!
//! A board's definition can describe its layout as `fragments` placed by a
//! `composition` of instances (up to [`MAX_INSTANCES`]). For each instance the
//! board reports which fragment the hardware detected, and stores which of the
//! instance's `fragment_options` the user picked.
//!
//! ```text
//! FRAGMENT_GET_HARDWARE   [0x18]                      -> [0x18][count][fragment_id x21]
//! FRAGMENT_GET_SELECTIONS [0x19]                      -> [0x19][count][option_idx x21]
//! FRAGMENT_SET_SELECTIONS [0x1A][count][option x21]   -> [0x1A][status]   (0 = ok)
//! ```
//! `0xFF` means "not detected" / "no selection". SET replaces every instance's
//! selection, so send the current ones for the instances you don't change.

use super::{svil_payload, SvilCommand};

/// Instance slots in the fragment commands.
pub const MAX_INSTANCES: usize = 21;
/// No detection / no selection.
pub const NO_SELECTION: u8 = 0xFF;

/// `[0x18]`.
pub fn get_hardware_payload() -> Vec<u8> {
    svil_payload(SvilCommand::FragmentGetHardware, &[])
}

/// `[0x19]`.
pub fn get_selections_payload() -> Vec<u8> {
    svil_payload(SvilCommand::FragmentGetSelections, &[])
}

/// Per-instance values of a GET_HARDWARE or GET_SELECTIONS reply
/// `[echo][count][value x21]`, `None` for instances reporting `0xFF`.
/// `None` overall for a reply that is too short or answers another command.
pub fn parse_instance_bytes(reply: &[u8], cmd: SvilCommand) -> Option<Vec<Option<u8>>> {
    if reply.len() < 3 || reply[0] != cmd as u8 {
        return None;
    }
    let count = (reply[1] as usize).min(MAX_INSTANCES);
    Some(
        (0..count)
            .map(|i| reply.get(2 + i).copied().filter(|&v| v != NO_SELECTION))
            .collect(),
    )
}

/// Fragment IDs the hardware detected, per instance.
pub fn parse_hardware(reply: &[u8]) -> Option<Vec<Option<u8>>> {
    parse_instance_bytes(reply, SvilCommand::FragmentGetHardware)
}

/// Stored option indices (into each instance's `fragment_options`), per instance.
pub fn parse_selections(reply: &[u8]) -> Option<Vec<Option<u8>>> {
    parse_instance_bytes(reply, SvilCommand::FragmentGetSelections)
}

/// `[0x1A][count][option x21]` with the selections of the first `count`
/// instances (`None` = clear) and `0xFF` in the unused slots.
pub fn set_selections_payload(selections: &[Option<u8>]) -> Vec<u8> {
    let count = selections.len().min(MAX_INSTANCES);
    let mut args = vec![count as u8];
    args.extend((0..MAX_INSTANCES).map(|i| selections.get(i).copied().flatten().unwrap_or(NO_SELECTION)));
    svil_payload(SvilCommand::FragmentSetSelections, &args)
}

/// Whether a SET_SELECTIONS reply `[0x1A][status]` reports success.
pub fn set_selections_ok(reply: &[u8]) -> bool {
    reply.len() >= 2 && reply[0] == SvilCommand::FragmentSetSelections as u8 && reply[1] == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_and_selections() {
        let mut reply = vec![0x18, 3, 2, 0xFF, 7];
        reply.resize(2 + MAX_INSTANCES, 0xFF);
        assert_eq!(parse_hardware(&reply), Some(vec![Some(2), None, Some(7)]));
        assert_eq!(parse_selections(&reply), None);
        reply[0] = 0x19;
        assert_eq!(parse_selections(&reply), Some(vec![Some(2), None, Some(7)]));
        assert_eq!(parse_hardware(&[0x18, 1]), None);
    }

    #[test]
    fn set_payload() {
        let p = set_selections_payload(&[Some(1), None]);
        assert_eq!(p.len(), 2 + MAX_INSTANCES);
        assert_eq!(&p[..4], &[0x1A, 2, 1, 0xFF]);
        assert!(p[4..].iter().all(|&b| b == 0xFF));
        assert!(set_selections_ok(&[0x1A, 0]));
        assert!(!set_selections_ok(&[0x1A, 1]));
    }
}
