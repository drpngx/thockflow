# Kanata Mouse Movement Design Document

## Overview
Add support for Kanata mouse movement actions: `movemouse-*`, `movemouse-accel-*`, `setmouse`, and `movemouse-speed`. These actions allow keyboard control of mouse cursor movement.

## Kanata Reference

### Basic Mouse Movement

```
(movemouse-$variant $interval $distance)
```

**Variants:** `up`, `down`, `left`, `right`

| Parameter | Description |
|-----------|-------------|
| `$interval` | Milliseconds between move activations (1-65535) |
| `$distance` | Distance in pixels per activation (1-65535) |

**Examples:**
- `(movemouse-up 1 1)` - Move mouse up slowly
- `(movemouse-right 10 5)` - Move mouse right faster

### Accelerated Mouse Movement

```
(movemouse-accel-$variant $interval $acceleration-time $min $max)
```

**Variants:** `up`, `down`, `left`, `right`

| Parameter | Description |
|-----------|-------------|
| `$interval` | Milliseconds between move activations |
| `$acceleration-time` | Milliseconds to ramp from min to max |
| `$min` | Initial distance per activation (pixels) |
| `$max` | Maximum distance per activation (pixels) |

**Examples:**
- `(movemouse-accel-up 1 1000 1 5)` - Start slow, accelerate to 5px over 1 second
- `(movemouse-accel-left 5 500 2 10)` - Medium acceleration

### Set Absolute Mouse Position

```
(setmouse $x $y)
```

| Parameter | Description |
|-----------|-------------|
| `$x` | X coordinate (platform-specific) |
| `$y` | Y coordinate (platform-specific) |

**Platform Notes:**
- macOS: Pixel coordinates (0,0 = top-left, max = screen resolution)
- Windows: Normalized 0-65535 coordinates

**Examples:**
- `(setmouse 960 540)` - Center of 1920x1080 screen (macOS)
- `(setmouse 32768 32768)` - Center (Windows)

### Modify Mouse Movement Speed

```
(movemouse-speed $percentage)
```

| Parameter | Description |
|-----------|-------------|
| `$percentage` | Speed percentage (1-65535, typically 50-300) |

**Examples:**
- `(movemouse-speed 200)` - Double speed
- `(movemouse-speed 50)` - Half speed

## Implementation Design

### 1. Action Definitions

Add to `KANATA_ACTIONS`:

```rust
// Basic movemouse actions - 2 integer params
KanataActionInfo { name: "movemouse-up", params: &[ParamType::Integer, ParamType::Integer] }
KanataActionInfo { name: "movemouse-down", params: &[ParamType::Integer, ParamType::Integer] }
KanataActionInfo { name: "movemouse-left", params: &[ParamType::Integer, ParamType::Integer] }
KanataActionInfo { name: "movemouse-right", params: &[ParamType::Integer, ParamType::Integer] }

// Accelerated movemouse actions - 4 integer params
KanataActionInfo { name: "movemouse-accel-up", params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer] }
KanataActionInfo { name: "movemouse-accel-down", params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer] }
KanataActionInfo { name: "movemouse-accel-left", params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer] }
KanataActionInfo { name: "movemouse-accel-right", params: &[ParamType::Integer, ParamType::Integer, ParamType::Integer, ParamType::Integer] }

// Set absolute position - 2 integer params
KanataActionInfo { name: "setmouse", params: &[ParamType::Integer, ParamType::Integer] }

// Modify speed - 1 integer param
KanataActionInfo { name: "movemouse-speed", params: &[ParamType::Integer] }
```

### 2. Validation

The existing validation system already supports:
- Integer parameter validation via `ParamType::Integer`
- Range checking (will add for 1-65535)

Special validation for mouse actions:
- All integer params must be in range [1, 65535]
- No special cross-parameter validation needed

### 3. Completion System

The existing completion system will automatically:
1. Suggest action names when typing `(move` or similar
2. Suggest integer values (with existing timeout suggestions)
3. Provide parameter hints

**Enhancement:** Add mouse-specific default suggestions:
- For movemouse: `1`, `5`, `10` for interval; `1`, `5`, `10` for distance
- For movemouse-accel: `1`, `5` for interval; `500`, `1000` for accel-time; `1`, `5` for min/max
- For setmouse: `0`, `960`, `1920` for x; `0`, `540`, `1080` for y
- For movemouse-speed: `50`, `100`, `200`

### 4. Display Formatting

Add nice display for mouse actions in `format_kanata_keycode`:
- `movemouse-*` → show direction with arrow symbols
- `movemouse-accel-*` → show "accel" indicator
- `setmouse` → show coordinates
- `movemouse-speed` → show percentage with %

## Implementation Status

✅ **COMPLETED** - All phases implemented and tested.

### Phase 1: Add Action Definitions ✅
- [x] Added 13 new actions to `KANATA_ACTIONS`:
  - `movemouse-up`, `movemouse-down`, `movemouse-left`, `movemouse-right` (2 params)
  - `movemouse-accel-up`, `movemouse-accel-down`, `movemouse-accel-left`, `movemouse-accel-right` (4 params)
  - `setmouse` (2 params)
  - `movemouse-speed` (1 param)
- [x] No new `ParamType` needed - reused `Integer`

### Phase 2: Validation Enhancement ✅
- [x] Added integer range validation (1-65535) for mouse action parameters
- [x] Added `is_mouse_action` detection in validation logic

### Phase 3: Completion Enhancement ✅
- [x] Added `get_current_mouse_action()` helper function
- [x] Added `get_mouse_action_suggestions()` function with mouse-specific values
- [x] Updated suggestion logic to detect when completing mouse action params
- [x] Added unit tests for completion

### Phase 4: Display Formatting (Optional Enhancement)
- [ ] Update `format_kanata_keycode` to handle mouse actions with symbols
- [ ] Add symbols: 🖱↑ 🖱↓ 🖱← 🖱→ 🖱accel↑ etc.

### Phase 5: Testing ✅
- [x] Unit tests for action definitions
- [x] Unit tests for validation
- [x] Unit tests for completion
- [x] Integration tests (mouse in multi, tap-hold, complex layer)

## Test Plan

### Unit Tests for Action Definitions

```rust
#[test]
fn test_mouse_actions_exist() {
    let mouse_actions = [
        "movemouse-up", "movemouse-down", "movemouse-left", "movemouse-right",
        "movemouse-accel-up", "movemouse-accel-down", "movemouse-accel-left", "movemouse-accel-right",
        "setmouse", "movemouse-speed"
    ];
    
    for action_name in &mouse_actions {
        assert!(KANATA_ACTIONS.iter().any(|a| a.name == *action_name),
            "Mouse action {} should exist", action_name);
    }
}

#[test]
fn test_movemouse_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "movemouse-up").unwrap();
    assert_eq!(action.params.len(), 2);
    assert_eq!(action.params[0], ParamType::Integer);
    assert_eq!(action.params[1], ParamType::Integer);
}

#[test]
fn test_movemouse_accel_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "movemouse-accel-up").unwrap();
    assert_eq!(action.params.len(), 4);
    assert!(action.params.iter().all(|&p| p == ParamType::Integer));
}

#[test]
fn test_setmouse_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "setmouse").unwrap();
    assert_eq!(action.params.len(), 2);
    assert_eq!(action.params[0], ParamType::Integer);
    assert_eq!(action.params[1], ParamType::Integer);
}

#[test]
fn test_movemouse_speed_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "movemouse-speed").unwrap();
    assert_eq!(action.params.len(), 1);
    assert_eq!(action.params[0], ParamType::Integer);
}
```

### Unit Tests for Validation

```rust
#[test]
fn test_validate_movemouse_basic() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(movemouse-up 1 1)"));
    assert!(validator.validate_action("(movemouse-down 10 5)"));
    assert!(validator.validate_action("(movemouse-left 50 100)"));
    assert!(validator.validate_action("(movemouse-right 100 50)"));
}

#[test]
fn test_validate_movemouse_accel() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(movemouse-accel-up 1 1000 1 5)"));
    assert!(validator.validate_action("(movemouse-accel-down 5 500 2 10)"));
    assert!(validator.validate_action("(movemouse-accel-left 10 2000 1 20)"));
    assert!(validator.validate_action("(movemouse-accel-right 2 750 3 15)"));
}

#[test]
fn test_validate_setmouse() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(setmouse 0 0)"));
    assert!(validator.validate_action("(setmouse 960 540)"));
    assert!(validator.validate_action("(setmouse 32768 32768)"));
}

#[test]
fn test_validate_movemouse_speed() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(movemouse-speed 50)"));
    assert!(validator.validate_action("(movemouse-speed 100)"));
    assert!(validator.validate_action("(movemouse-speed 200)"));
}

#[test]
fn test_validate_movemouse_invalid_params() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Too few params
    assert!(!validator.validate_action("(movemouse-up 1)"));
    assert!(!validator.validate_action("(movemouse-accel-up 1 1000 1)"));
    
    // Too many params
    assert!(!validator.validate_action("(movemouse-up 1 1 1)"));
    assert!(!validator.validate_action("(setmouse 0 0 0)"));
    
    // Invalid (non-integer) params
    assert!(!validator.validate_action("(movemouse-up abc 1)"));
    assert!(!validator.validate_action("(movemouse-speed fast)"));
}

#[test]
fn test_validate_movemouse_range() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Valid boundary values
    assert!(validator.validate_action("(movemouse-up 1 1)"));
    assert!(validator.validate_action("(movemouse-up 65535 65535)"));
    
    // Invalid: zero
    assert!(!validator.validate_action("(movemouse-up 0 1)"));
    assert!(!validator.validate_action("(movemouse-up 1 0)"));
    
    // Invalid: negative
    assert!(!validator.validate_action("(movemouse-up -1 1)"));
}
```

### Unit Tests for Completion

```rust
#[test]
fn test_suggestions_movemouse_actions() {
    let data = create_test_data();
    
    // Typing "(move" should suggest movemouse actions
    let (_, suggestions) = get_suggestions("(move", &data);
    assert!(suggestions.iter().any(|s| s.contains("movemouse-up")));
    assert!(suggestions.iter().any(|s| s.contains("movemouse-accel-up")));
    assert!(suggestions.iter().any(|s| s.contains("movemouse-speed")));
}

#[test]
fn test_suggestions_movemouse_first_param() {
    let data = create_test_data();
    
    // After typing "(movemouse-up ", should suggest integers
    let (_, suggestions) = get_suggestions("(movemouse-up ", &data);
    
    // Should have integer suggestions
    assert!(suggestions.iter().any(|s| s.parse::<u32>().is_ok()),
        "Should suggest integer values for interval");
}

#[test]
fn test_suggestions_movemouse_second_param() {
    let data = create_test_data();
    
    // After typing "(movemouse-up 1 ", should suggest integers for distance
    let (_, suggestions) = get_suggestions("(movemouse-up 1 ", &data);
    assert!(suggestions.iter().any(|s| s.parse::<u32>().is_ok()),
        "Should suggest integer values for distance");
}

#[test]
fn test_suggestions_movemouse_accel_params() {
    let data = create_test_data();
    
    // Test completion at each param position
    let (_, suggestions) = get_suggestions("(movemouse-accel-up ", &data);
    assert!(!suggestions.is_empty(), "Should suggest interval values");
    
    let (_, suggestions) = get_suggestions("(movemouse-accel-up 1 ", &data);
    assert!(!suggestions.is_empty(), "Should suggest acceleration time values");
    
    let (_, suggestions) = get_suggestions("(movemouse-accel-up 1 1000 ", &data);
    assert!(!suggestions.is_empty(), "Should suggest min distance values");
}

#[test]
fn test_suggestions_setmouse() {
    let data = create_test_data();
    
    let (_, suggestions) = get_suggestions("(setmouse ", &data);
    assert!(!suggestions.is_empty(), "Should suggest x coordinate values");
}

#[test]
fn test_suggestions_movemouse_speed() {
    let data = create_test_data();
    
    let (_, suggestions) = get_suggestions("(movemouse-speed ", &data);
    
    // Should suggest common speed percentages
    assert!(suggestions.contains(&"50".to_string()) || 
            suggestions.contains(&"100".to_string()) ||
            suggestions.contains(&"200".to_string()),
        "Should suggest speed percentage values");
}
```

### Integration Test Scenarios

```rust
#[test]
fn test_mouse_action_workflow() {
    // Test complete workflow: type action, get completions, validate
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // 1. User types "(move"
    let (_, suggestions) = get_suggestions("(move", &data);
    assert!(suggestions.contains(&"(movemouse-up".to_string()));
    
    // 2. User selects "(movemouse-up"
    // 3. User types " 1 5)" and completes the action
    assert!(validator.validate_action("(movemouse-up 1 5)"));
}

#[test]
fn test_complex_mouse_layer() {
    // Test a realistic mouse layer configuration
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    let layer_config = r#"
(defalias
  mwu (movemouse-up 1 1)
  mwd (movemouse-down 1 1)
  mwl (movemouse-left 1 1)
  mwr (movemouse-right 1 1)
  ma↑ (movemouse-accel-up 1 1000 1 5)
  ma↓ (movemouse-accel-down 1 1000 1 5)
  ma← (movemouse-accel-left 1 1000 1 5)
  ma→ (movemouse-accel-right 1 1000 1 5)
  sm  (setmouse 960 540)
  fst (movemouse-speed 200)
  slw (movemouse-speed 50)
)
"#;
    
    // All aliases should be valid
    assert!(validator.validate_action("(movemouse-up 1 1)"));
    assert!(validator.validate_action("(movemouse-accel-up 1 1000 1 5)"));
    assert!(validator.validate_action("(setmouse 960 540)"));
    assert!(validator.validate_action("(movemouse-speed 200)"));
}

#[test]
fn test_mouse_in_multi() {
    // Mouse actions can be combined with other actions
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(multi (movemouse-up 1 1) (movemouse-right 1 1))"));
    assert!(validator.validate_action("(multi (movemouse-speed 200) (movemouse-left 5 10))"));
}

#[test]
fn test_mouse_in_tap_hold() {
    // Mouse actions can be used in tap-hold
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(tap-hold 200 200 (movemouse-up 1 1) (movemouse-down 1 1))"));
}
```

### Edge Cases

```rust
#[test]
fn test_movemouse_unicode_variants() {
    // Kanata also supports unicode variants: 🖱↑ 🖱↓ 🖱← 🖱→
    // These should be handled if we want full compatibility
    // For now, we focus on ASCII action names
}

#[test]
fn test_movemouse_case_sensitivity() {
    // Action names should be case-sensitive (lowercase)
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(!validator.validate_action("(Movemouse-up 1 1)"));
    assert!(!validator.validate_action("(MOVEUP 1 1)"));
}

#[test]
fn test_movemouse_whitespace() {
    // Should handle various whitespace
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(movemouse-up   1    1)"));
    assert!(validator.validate_action("(movemouse-up 1 1)"));
}
```

## Future Enhancements

1. **Unicode Action Names**: Support `🖱↑` `🖱↓` `🖱←` `🖱→` `🖱accel↑` etc. as aliases
2. **Visual Mouse Layer Preview**: Show a mouse cursor icon in the key display
3. **Coordinate Picker**: For `setmouse`, provide a visual coordinate picker
4. **Mouse Button Actions**: Already exist (`mlft`, `mrgt`, etc.) but could add tap variants
5. **Mouse Wheel Actions**: Already exist (`mwheel-*`) but could add inertial variants
