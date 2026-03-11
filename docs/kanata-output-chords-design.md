# Kanata Output Chords Design Document

## Overview
Implement support for Kanata output chords (pre-modified keys) like `C-S-a` which outputs Ctrl+Shift+a. This feature allows prefixing key names with modifier strings to create chord outputs.

## Kanata Reference
From the Kanata docs, the following prefixes are supported:

| Prefix | Meaning |
|--------|---------|
| `C-` | Left Control |
| `RC-` | Right Control |
| `A-` | Left Alt |
| `RA-` | Right Alt (AltGr) |
| `AG-` | Also Right Alt/AltGr |
| `S-` | Left Shift |
| `RS-` | Right Shift |
| `M-` | Left Meta |
| `RM-` | Right Meta |

Multiple prefixes can be combined (e.g., `C-S-a`). Duplicate prefixes are not allowed.

## Implementation Design

### 1. Data Structures

```rust
/// Output chord modifiers
const OUTPUT_CHORD_MODIFIERS: &[&str] = &[
    "C-", "RC-",   // Control variants
    "A-", "RA-", "AG-",  // Alt variants  
    "S-", "RS-",   // Shift variants
    "M-", "RM-",   // Meta variants
];

/// Parse an output chord into its components
struct OutputChord {
    modifiers: Vec<String>, // e.g., ["C-", "S-"]
    key: String,            // e.g., "a"
}
```

### 2. Parser Functions

```rust
/// Parse an output chord string into modifiers and base key
/// Returns None if not a valid output chord format
fn parse_output_chord(input: &str) -> Option<OutputChord>

/// Check if a string is a valid modifier prefix
fn is_modifier_prefix(s: &str) -> bool

/// Get the base key from a potential output chord (strips all modifier prefixes)
fn get_base_key(input: &str) -> &str

/// Check if a string has any modifier prefix
fn has_modifier_prefix(input: &str) -> bool

/// Get valid next modifiers given current prefixes (prevents duplicates)
fn get_next_modifiers(current_prefixes: &[String]) -> Vec<&'static str>
```

### 3. Completion System Changes

The completion system needs to support progressive completion:

**Case 1: Empty query or query ends with `-`**
- If user types `C-`, suggest:
  - More modifiers: `C-S-`, `C-A-`, `C-M-`, `C-RC-`, etc.
  - Base keys: `C-a`, `C-b`, `C-esc`, etc.
  
**Case 2: Query has partial base key**
- If user types `C-a`, suggest:
  - Matching base keys with prefix: `C-a`, `C-b`, `C-c`, etc.

**Case 3: Query is just a modifier prefix**
- If user types `C`, suggest `C-` as a completion option

### 4. Validation Changes

Update `validate_action` to accept output chords:

```rust
fn validate_action(&self, text: &str) -> bool {
    // ... existing validation ...
    
    // Check for output chord
    if has_modifier_prefix(text) {
        let base = get_base_key(text);
        // Validate no duplicate modifiers
        // Validate base key is a valid KANATA_KEY
        return KANATA_KEYS.contains(&base);
    }
    
    // ... rest of validation ...
}
```

### 5. Display Formatting

Output chords should be displayed nicely:
- `C-a` → "Ctrl+A" or keep as "C-a"
- `C-S-tab` → "Ctrl+Shift+Tab"
- `M-spc` → "Cmd+Space" (Mac) or "Win+Space" (Windows)

### 6. UI/UX Considerations

- Suggestions should show modifier prefixes with visual distinction
- When user selects a modifier prefix (e.g., `C-`), the input should update and new suggestions should appear
- Base keys should be filtered to show only those matching after the prefixes

## Implementation Status

✅ **COMPLETED** - All phases implemented and tested.

### Phase 1: Core Parser ✅
- [x] Add `OUTPUT_CHORD_MODIFIERS` constant
- [x] Implement `parse_output_chord()` function
- [x] Implement helper functions: `extract_modifier_prefix()`, `get_base_key()`, `has_modifier_prefix()`, `get_current_modifiers()`, `has_duplicate_modifiers()`, `get_available_modifiers()`, `is_modifier_prefix_str()`
- [x] Add unit tests for parser functions

### Phase 2: Validation ✅
- [x] Update `validate_action()` to recognize output chords
- [x] Add duplicate modifier detection (including RA-/AG- equivalence)
- [x] Add unit tests for validation

### Phase 3: Completion System ✅
- [x] Modify `get_suggestions()` to suggest modifier prefixes
- [x] Implement progressive completion (prefix → more prefixes or keys)
- [x] Add filtering for base keys after prefixes
- [x] Add unit tests for completions

### Phase 4: Display (Optional Enhancement)
- [ ] Update `format_kanata_keycode()` to handle output chords with nice symbols

### Phase 5: Integration Testing ✅
- [x] Test full user flows
- [x] Edge cases covered in unit tests

## Test Plan

### Unit Tests for Parser

```rust
#[test]
fn test_parse_output_chord_simple() {
    let chord = parse_output_chord("C-a").unwrap();
    assert_eq!(chord.modifiers, vec!["C-"]);
    assert_eq!(chord.key, "a");
}

#[test]
fn test_parse_output_chord_multiple() {
    let chord = parse_output_chord("C-S-a").unwrap();
    assert_eq!(chord.modifiers, vec!["C-", "S-"]);
    assert_eq!(chord.key, "a");
}

#[test]
fn test_parse_output_chord_right_mods() {
    let chord = parse_output_chord("RC-RS-M-tab").unwrap();
    assert_eq!(chord.modifiers, vec!["RC-", "RS-", "M-"]);
    assert_eq!(chord.key, "tab");
}

#[test]
fn test_parse_output_chord_altgr() {
    let chord = parse_output_chord("AG-a").unwrap();
    assert_eq!(chord.modifiers, vec!["AG-"]);
    assert_eq!(chord.key, "a");
    
    let chord2 = parse_output_chord("RA-a").unwrap();
    assert_eq!(chord2.modifiers, vec!["RA-"]);
}

#[test]
fn test_parse_output_chord_not_a_chord() {
    assert!(parse_output_chord("a").is_none());
    assert!(parse_output_chord("esc").is_none());
    assert!(parse_output_chord("_").is_none());
}

#[test]
fn test_get_base_key() {
    assert_eq!(get_base_key("C-S-a"), "a");
    assert_eq!(get_base_key("C-esc"), "esc");
    assert_eq!(get_base_key("a"), "a");
    assert_eq!(get_base_key("M-spc"), "spc");
}

#[test]
fn test_has_modifier_prefix() {
    assert!(has_modifier_prefix("C-a"));
    assert!(has_modifier_prefix("S-tab"));
    assert!(!has_modifier_prefix("a"));
    assert!(!has_modifier_prefix("esc"));
}

#[test]
fn test_get_next_modifiers() {
    // After C-, we can add any modifier except C- again
    let next = get_next_modifiers(&["C-".to_string()]);
    assert!(next.contains(&"S-"));
    assert!(next.contains(&"A-"));
    assert!(next.contains(&"M-"));
    assert!(next.contains(&"RC-"));
    assert!(!next.contains(&"C-")); // No duplicates
}
```

### Unit Tests for Validation

```rust
#[test]
fn test_validate_output_chord_simple() {
    let data = KeymapData::default();
    let validator = KanataValidator::new(&data);
    assert!(validator.validate_action("C-a"));
    assert!(validator.validate_action("S-1"));
    assert!(validator.validate_action("M-tab"));
}

#[test]
fn test_validate_output_chord_multiple() {
    let data = KeymapData::default();
    let validator = KanataValidator::new(&data);
    assert!(validator.validate_action("C-S-a"));
    assert!(validator.validate_action("C-A-del"));
    assert!(validator.validate_action("C-S-M-a"));
}

#[test]
fn test_validate_output_chord_invalid_base() {
    let data = KeymapData::default();
    let validator = KanataValidator::new(&data);
    assert!(!validator.validate_action("C-invalidkey"));
}

#[test]
fn test_validate_output_chord_duplicate() {
    let data = KeymapData::default();
    let validator = KanataValidator::new(&data);
    // Duplicate C- should be invalid
    assert!(!validator.validate_action("C-C-a"));
    assert!(!validator.validate_action("S-C-S-a"));
}

#[test]
fn test_validate_output_chord_all_modifiers() {
    let data = KeymapData::default();
    let validator = KanataValidator::new(&data);
    // Test all modifier variants
    assert!(validator.validate_action("C-a"));
    assert!(validator.validate_action("RC-a"));
    assert!(validator.validate_action("A-a"));
    assert!(validator.validate_action("RA-a"));
    assert!(validator.validate_action("AG-a"));
    assert!(validator.validate_action("S-a"));
    assert!(validator.validate_action("RS-a"));
    assert!(validator.validate_action("M-a"));
    assert!(validator.validate_action("RM-a"));
}
```

### Unit Tests for Completion

```rust
#[test]
fn test_suggestions_modifier_prefix() {
    let data = KeymapData::default();
    let (prefix, suggestions) = get_suggestions("C", &data);
    assert!(suggestions.contains(&"C-".to_string()));
}

#[test]
fn test_suggestions_after_prefix() {
    let data = KeymapData::default();
    let (prefix, suggestions) = get_suggestions("C-", &data);
    // Should suggest more modifiers and base keys
    assert!(suggestions.contains(&"C-S-".to_string()));
    assert!(suggestions.contains(&"C-a".to_string()));
    assert!(suggestions.contains(&"C-esc".to_string()));
}

#[test]
fn test_suggestions_multiple_prefixes() {
    let data = KeymapData::default();
    let (prefix, suggestions) = get_suggestions("C-S-", &data);
    // After C-S-, can add A-, M-, RC-, RS-, RM-, AG-, RA- but not C- or S-
    assert!(suggestions.contains(&"C-S-a".to_string()));
    assert!(suggestions.contains(&"C-S-M-".to_string()));
    assert!(!suggestions.contains(&"C-S-C-".to_string())); // No duplicate
    assert!(!suggestions.contains(&"C-S-S-".to_string())); // No duplicate
}

#[test]
fn test_suggestions_partial_base() {
    let data = KeymapData::default();
    let (prefix, suggestions) = get_suggestions("C-a", &data);
    // Should suggest base keys starting with 'a' with C- prefix
    assert!(suggestions.contains(&"C-a".to_string()));
    assert!(!suggestions.contains(&"C-b".to_string())); // Should filter to 'a' keys
}

#[test]
fn test_suggestions_all_modifiers() {
    let data = KeymapData::default();
    let modifiers = ["C", "RC", "A", "RA", "AG", "S", "RS", "M", "RM"];
    for m in &modifiers {
        let query = format!("{}", m);
        let (prefix, suggestions) = get_suggestions(&query, &data);
        assert!(
            suggestions.contains(&format!("{}-", m)),
            "Should suggest {}- for query {}",
            m,
            query
        );
    }
}
```

### Integration Test Scenarios

```rust
#[test]
fn test_output_chord_workflow_simple() {
    // User wants to type C-a
    // 1. User types 'C'
    //    - Suggestions include: C-
    // 2. User selects 'C-'
    //    - Input becomes 'C-'
    //    - Suggestions include: C-a, C-b, C-S-, C-M-, etc.
    // 3. User types 'a' or selects 'C-a'
    //    - Input becomes 'C-a'
    //    - Validation passes
}

#[test]
fn test_output_chord_workflow_complex() {
    // User wants to type C-S-tab
    // 1. User types 'C'
    //    - Select 'C-'
    // 2. Input: 'C-'
    //    - Select 'C-S-'
    // 3. Input: 'C-S-'
    //    - Type 'ta' or select 'C-S-tab'
    // 4. Input: 'C-S-tab'
    //    - Validation passes
}

#[test]
fn test_output_chord_in_action() {
    // Output chords should work inside actions
    // (tap-hold 200 200 C-a C-S-a)
    // (multi C-c C-v)
}
```

## Edge Cases

1. **Duplicate modifiers**: `C-C-a` should be invalid
2. **Empty after prefix**: `C-` alone (without base key) is incomplete but acceptable during typing
3. **Unknown base key**: `C-unknown` should be invalid
4. **Case sensitivity**: Kanata is case-insensitive for key names; we should match that
5. **Unicode variants**: Some modifiers have unicode variants like `‹⎈` for C-, but we focus on ASCII versions
6. **AG- vs RA-**: Both mean Right Alt/AltGr; treat as equivalent for duplicate detection

## Future Enhancements

1. Support unicode modifier indicators (e.g., `‹⎈` for left control)
2. Smart display: `C-c` → "Copy", `C-v` → "Paste" on context
3. Visual modifier indicators in key display (use symbols like ⌃⌥⇧⌘)
4. Chord preview: show what the chord outputs while typing
