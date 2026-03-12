//! Layout type detection for ZMK keymaps
//!
//! Detects whether a keymap uses QWERTY, Dvorak, Colemak, or other layouts
//! based on the position of known keys in the bindings.

use serde::{Deserialize, Serialize};

/// Detected keyboard layout type
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum DetectedKeyboardLayout {
    #[default]
    Qwerty,
    Dvorak,
    Colemak,
    Workman,
    Unknown,
}

impl DetectedKeyboardLayout {
    /// Get the display name for this layout type
    pub fn display_name(&self) -> &'static str {
        match self {
            DetectedKeyboardLayout::Qwerty => "QWERTY",
            DetectedKeyboardLayout::Dvorak => "Dvorak",
            DetectedKeyboardLayout::Colemak => "Colemak",
            DetectedKeyboardLayout::Workman => "Workman",
            DetectedKeyboardLayout::Unknown => "Unknown",
        }
    }
}

/// Result of layout detection
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LayoutDetectionResult {
    pub layout_type: DetectedKeyboardLayout,
    pub confidence: f32,
}

/// Extract keypresses from bindings (extracts XXX from &kp XXX)
pub fn extract_keypresses(bindings: &[String]) -> Vec<String> {
    bindings
        .iter()
        .filter_map(|binding| {
            let parts: Vec<&str> = binding.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == "&kp" {
                Some(parts[1].to_uppercase())
            } else {
                None
            }
        })
        .collect()
}

/// Detect layout type based on the position of known keys
/// Returns the detected layout type and confidence score (0.0 - 1.0)
pub fn detect_layout_type(bindings: &[String]) -> LayoutDetectionResult {
    let keys = extract_keypresses(bindings);
    
    if keys.is_empty() {
        return LayoutDetectionResult {
            layout_type: DetectedKeyboardLayout::Unknown,
            confidence: 0.0,
        };
    }

    // Get first 12 keys (typically the alpha row: Q-P or equivalent)
    let sample: Vec<&str> = keys.iter().take(12).map(|s| s.as_str()).collect();
    
    // Define expected positions for each layout
    // QWERTY: Q W E R T Y U I O P [ ]
    let qwerty_pattern = ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"];
    // Dvorak: ' , . P Y F G C R L / =
    let dvorak_pattern = ["SQT", "COMMA", "DOT", "P", "Y", "F", "G", "C", "R", "L"];
    // Colemak: Q W F P G J L U Y ; [
    let colemak_pattern = ["Q", "W", "F", "P", "G", "J", "L", "U", "Y", "SEMI"];
    // Workman: Q D R W B J F U P ; [
    let workman_pattern = ["Q", "D", "R", "W", "B", "J", "F", "U", "P", "SEMI"];

    let qwerty_score = calculate_match_score(&sample, &qwerty_pattern);
    let dvorak_score = calculate_match_score(&sample, &dvorak_pattern);
    let colemak_score = calculate_match_score(&sample, &colemak_pattern);
    let workman_score = calculate_match_score(&sample, &workman_pattern);

    // Find the highest scoring layout
    let scores = [
        (DetectedKeyboardLayout::Qwerty, qwerty_score),
        (DetectedKeyboardLayout::Dvorak, dvorak_score),
        (DetectedKeyboardLayout::Colemak, colemak_score),
        (DetectedKeyboardLayout::Workman, workman_score),
    ];

    let (best_layout, best_score) = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .copied()
        .unwrap_or((DetectedKeyboardLayout::Unknown, 0.0));

    // Calculate confidence based on how much better the best is than the second best
    let second_best = scores
        .iter()
        .filter(|(l, _)| *l != best_layout)
        .map(|(_, s)| *s)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    // Confidence is based on the score difference
    let confidence = if best_score > 0.0 {
        (best_score - second_best).min(1.0).max(0.0)
    } else {
        0.0
    };

    // If confidence is too low, return Unknown
    let layout_type = if confidence < 0.3 {
        DetectedKeyboardLayout::Unknown
    } else {
        best_layout
    };

    LayoutDetectionResult {
        layout_type,
        confidence: best_score, // Use raw score as confidence for matched layout
    }
}

/// Calculate how well the sample matches the expected pattern
/// Returns a score between 0.0 and 1.0
fn calculate_match_score(sample: &[&str], expected: &[&str]) -> f32 {
    if sample.len() < expected.len() {
        return 0.0;
    }

    let mut matches = 0;
    for (i, expected_key) in expected.iter().enumerate() {
        if sample[i] == *expected_key {
            matches += 1;
        }
    }

    matches as f32 / expected.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_qwerty() {
        let bindings = vec![
            "&kp Q".to_string(),
            "&kp W".to_string(),
            "&kp E".to_string(),
            "&kp R".to_string(),
            "&kp T".to_string(),
            "&kp Y".to_string(),
            "&kp U".to_string(),
            "&kp I".to_string(),
            "&kp O".to_string(),
            "&kp P".to_string(),
        ];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Qwerty));
        assert!(result.confidence > 0.8);
    }

    #[test]
    fn test_detect_dvorak() {
        let bindings = vec![
            "&kp SQT".to_string(),
            "&kp COMMA".to_string(),
            "&kp DOT".to_string(),
            "&kp P".to_string(),
            "&kp Y".to_string(),
            "&kp F".to_string(),
            "&kp G".to_string(),
            "&kp C".to_string(),
            "&kp R".to_string(),
            "&kp L".to_string(),
        ];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Dvorak));
        assert!(result.confidence > 0.8);
    }

    #[test]
    fn test_detect_colemak() {
        let bindings = vec![
            "&kp Q".to_string(),
            "&kp W".to_string(),
            "&kp F".to_string(),
            "&kp P".to_string(),
            "&kp G".to_string(),
            "&kp J".to_string(),
            "&kp L".to_string(),
            "&kp U".to_string(),
            "&kp Y".to_string(),
            "&kp SEMI".to_string(),
        ];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Colemak));
        assert!(result.confidence > 0.8);
    }

    #[test]
    fn test_detect_workman() {
        let bindings = vec![
            "&kp Q".to_string(),
            "&kp D".to_string(),
            "&kp R".to_string(),
            "&kp W".to_string(),
            "&kp B".to_string(),
            "&kp J".to_string(),
            "&kp F".to_string(),
            "&kp U".to_string(),
            "&kp P".to_string(),
            "&kp SEMI".to_string(),
        ];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Workman));
        assert!(result.confidence > 0.8);
    }

    #[test]
    fn test_detect_unknown_empty() {
        let bindings: Vec<String> = vec![];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Unknown));
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_detect_unknown_no_kp() {
        let bindings = vec![
            "&mo 1".to_string(),
            "&trans".to_string(),
            "&none".to_string(),
        ];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Unknown));
    }

    #[test]
    fn test_extract_keypresses() {
        let bindings = vec![
            "&kp Q".to_string(),
            "&mo 1".to_string(),
            "&kp W".to_string(),
            "&trans".to_string(),
            "&kp E".to_string(),
        ];
        let keys = extract_keypresses(&bindings);
        assert_eq!(keys, vec!["Q", "W", "E"]);
    }

    #[test]
    fn test_qwerty_partial_match() {
        // Mix of QWERTY and some other keys - should still detect as QWERTY
        let bindings = vec![
            "&kp Q".to_string(),
            "&kp W".to_string(),
            "&kp E".to_string(),
            "&kp R".to_string(),
            "&kp T".to_string(),
            "&kp Y".to_string(), // QWERTY pattern
            "&kp N1".to_string(),
            "&kp N2".to_string(),
            "&kp ESC".to_string(),
            "&kp TAB".to_string(),
        ];
        let result = detect_layout_type(&bindings);
        assert!(matches!(result.layout_type, DetectedKeyboardLayout::Qwerty));
    }

    #[test]
    fn test_display_name() {
        assert_eq!(DetectedKeyboardLayout::Qwerty.display_name(), "QWERTY");
        assert_eq!(DetectedKeyboardLayout::Dvorak.display_name(), "Dvorak");
        assert_eq!(DetectedKeyboardLayout::Colemak.display_name(), "Colemak");
        assert_eq!(DetectedKeyboardLayout::Workman.display_name(), "Workman");
        assert_eq!(DetectedKeyboardLayout::Unknown.display_name(), "Unknown");
    }
}
