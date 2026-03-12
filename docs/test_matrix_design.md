# Test Matrix Feature Design & Implementation Document

## Overview

The Test Matrix feature in the Vial tab provides a visual representation of the physical keyboard layout that responds to real-time key presses. It helps users verify that their keyboard's switch matrix is working correctly and visualize layer switching behavior.

## Current State

The current implementation (`render_matrix_tab` in `src/vial/mod.rs`) provides a basic grid display:
- Shows a simple grid of squares representing matrix positions
- Uses green color for pressed keys, gray for unpressed
- Does NOT use the physical key layout from the Vial definition
- Does NOT handle layer switching
- Does NOT poll for matrix state updates

## Requirements

### Functional Requirements

1. **Physical Layout Rendering**: Draw keys using the actual physical layout positions from the Vial definition (same as Keymap tab)
2. **Real-time Highlighting**: Highlight keys when pressed, remove highlight on release
3. **Layer Display**: Show the keymap of the currently active layer
4. **Layer Switching**: When a layer-switching key is pressed, display the target layer
5. **Polling**: Poll the keyboard matrix state every 30ms when the Test Matrix tab is active

### Visual Requirements

1. Use the same physical layout rendering as the Keymap tab
2. Show key labels from the current layer's keymap
3. Apply visual highlight (green background + shadow) when matrix position is active
4. Smooth transitions for highlight state changes

## Design

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Test Matrix Component                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐  │
│  │  Polling     │─────▶│  Matrix      │─────▶│   Layout     │  │
│  │  Timer       │      │  State       │      │   Renderer   │  │
│  │  (30ms)      │      │  Handler     │      │              │  │
│  └──────────────┘      └──────────────┘      └──────────────┘  │
│                               │                         │       │
│                               ▼                         ▼       │
│                        ┌──────────────┐      ┌──────────────┐  │
│                        │  Layer       │─────▶│   Key        │  │
│                        │  Resolver    │      │   Widget     │  │
│                        └──────────────┘      └──────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Polling Loop** (30ms interval):
   - Send `GetKeyboardValue(SwitchMatrixState)` command via WebHID
   - Parse response using `vial_protocol::parse_matrix_state()`
   - Update `matrix_state` state variable

2. **Layer Resolution**:
   - Check if any pressed key has a layer-switching keycode
   - Resolve the effective layer based on pressed layer keys
   - Update `displayed_layer` state

3. **Rendering**:
   - Use `key_layout` (physical positions from Vial definition)
   - For each key position, check if matrix[row][col] is pressed
   - Render key with label from `layers[displayed_layer][key_index]`
   - Apply highlight styling if matrix position is pressed

### State Management

```rust
// Existing states (already in VialHome):
let matrix_state: UseStateHandle<Vec<Vec<bool>>>;  // From keyboard polling
let key_layout: UseStateHandle<Vec<MatrixPos>>;    // From Vial definition
let layers: UseStateHandle<Vec<Vec<u16>>>;         // Keycodes per layer
let active_layer: UseStateHandle<u8>;              // User-selected layer

// New state for Test Matrix:
let displayed_layer: UseStateHandle<u8>;           // Layer to display (may differ from active_layer)
let is_polling: UseStateHandle<bool>;              // Polling active flag
```

### Layer Resolution Logic

```rust
/// Determine which layer to display based on pressed keys
fn resolve_display_layer(
    matrix_state: &Vec<Vec<bool>>,
    layers: &Vec<Vec<u16>>,
    base_layer: u8,
    key_layout: &Vec<MatrixPos>,
    matrix_cols: u8,
    protocol_version: u32,
) -> u8 {
    // Priority order for layer switching (QMK behavior):
    // 1. TO (immediate layer switch)
    // 2. MO/DF/TG/OSL (momentary/default/toggle/one-shot layer)
    // 3. LT (layer tap - when held)
    
    for (layout_idx, pos) in key_layout.iter().enumerate() {
        if matrix_state[pos.row as usize][pos.col as usize] {
            let keycode_idx = (pos.row as usize) * (matrix_cols as usize) + (pos.col as usize);
            let keycode = layers[base_layer as usize][keycode_idx];
            
            if let Some((func, target_layer)) = layer_info(keycode, protocol_version) {
                match func {
                    "TO" => return target_layer,  // Immediate switch
                    "MO" | "DF" | "TG" | "OSL" | "TT" => return target_layer,
                    "LT" => return target_layer,  // When held
                    _ => {}
                }
            }
        }
    }
    
    base_layer
}
```

## Implementation Plan

### Phase 1: Polling Infrastructure

1. Add polling effect that runs when Test Matrix tab is active
2. Implement cleanup to stop polling when tab changes or disconnect
3. Handle errors gracefully (stop polling on error)

### Phase 2: Enhanced Rendering

1. Create new `render_test_matrix_tab` function
2. Reuse physical layout rendering from `render_keymap_tab`
3. Add highlight state based on matrix_state
4. Show key labels from current layer

### Phase 3: Layer Switching

1. Implement layer resolution logic
2. Add `displayed_layer` state
3. Update rendering to use resolved layer

## Code Changes

### File: `src/vial/mod.rs`

#### Add New State Variables (in VialHome)
```rust
let displayed_layer = use_state(|| 0u8);
let is_polling = use_state(|| false);
```

#### Add Polling Effect
```rust
// Matrix polling effect - only active on TestMatrix tab
{
    let device = device.clone();
    let matrix_state = matrix_state.clone();
    let active_tab = active_tab.clone();
    let matrix_rows = matrix_rows.clone();
    let matrix_cols = matrix_cols.clone();
    let is_polling = is_polling.clone();
    let error = error.clone();
    
    use_effect(move || {
        if *active_tab != VialTab::TestMatrix || device.is_none() {
            is_polling.set(false);
            return || ();
        }
        
        is_polling.set(true);
        let device = device.clone();
        
        let closure = Closure::wrap(Box::new(move || {
            if let Some(dev) = &*device {
                let matrix_state = matrix_state.clone();
                let rows = *matrix_rows;
                let cols = *matrix_cols;
                let error = error.clone();
                
                spawn_local(async move {
                    match webhid::send_message(
                        dev,
                        vial_protocol::VialMessage::get_switch_matrix_state().as_bytes(),
                    ).await {
                        Ok(resp) => {
                            let new_state = vial_protocol::parse_matrix_state(&resp, rows, cols);
                            matrix_state.set(new_state);
                        }
                        Err(e) => {
                            log::warn!("Matrix poll error: {e}");
                            // Optionally stop polling on persistent errors
                        }
                    }
                });
            }
        }) as Box<dyn FnMut()>);
        
        // Set up 30ms interval
        let window = web_sys::window().unwrap();
        let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            30,
        ).unwrap();
        
        move || {
            window.clear_interval_with_handle(interval_id);
            drop(closure);
        }
    });
}
```

#### Update Tab Rendering
```rust
VialTab::TestMatrix => render_test_matrix_tab(
    &matrix_state,
    &key_layout,
    &layers,
    &displayed_layer,
    *matrix_cols,
    *vial_protocol_ver,
    &*keyboard_name,
),
```

#### New Render Function
```rust
fn render_test_matrix_tab(
    matrix_state: &UseStateHandle<Vec<Vec<bool>>>,
    key_layout: &UseStateHandle<Vec<MatrixPos>>,
    layers: &UseStateHandle<Vec<Vec<u16>>>,
    displayed_layer: &UseStateHandle<u8>,
    matrix_cols: u8,
    protocol_version: u32,
    keyboard_name: &str,
) -> Html {
    // Resolve which layer to display based on pressed keys
    let display_layer = resolve_display_layer(
        matrix_state,
        layers,
        **displayed_layer,
        key_layout,
        matrix_cols,
        protocol_version,
    );
    
    let layer_keys = layers.get(display_layer as usize);
    
    // Render using physical layout with matrix state highlights
    // ... (similar to render_keymap_tab but with highlight based on matrix_state)
}
```

## Exhaustive Test List

### Unit Tests (vial-protocol crate)

#### Matrix State Parsing
```rust
#[test]
fn parse_matrix_state_single_key() {
    // Single key at row 0, col 0 pressed
}

#[test]
fn parse_matrix_state_multiple_keys_same_row() {
    // Multiple keys in same row pressed
}

#[test]
fn parse_matrix_state_multiple_keys_different_rows() {
    // Keys in different rows pressed
}

#[test]
fn parse_matrix_state_all_keys_pressed() {
    // All positions in matrix pressed
}

#[test]
fn parse_matrix_state_no_keys_pressed() {
    // Empty matrix
}

#[test]
fn parse_matrix_state_large_matrix() {
    // Test with 12x20 matrix (240 keys)
}

#[test]
fn parse_matrix_state_non_standard_cols() {
    // Test with columns not divisible by 8
}
```

### Integration Tests (src/vial/mod.rs)

#### Layer Resolution Tests
```rust
#[test]
fn resolve_display_layer_no_keys_pressed() {
    // Should return base layer when no keys pressed
}

#[test]
fn resolve_display_layer_mo_key_pressed() {
    // MO(1) pressed should display layer 1
}

#[test]
fn resolve_display_layer_to_key_pressed() {
    // TO(2) pressed should display layer 2
}

#[test]
fn resolve_display_layer_df_key_pressed() {
    // DF(3) pressed should display layer 3
}

#[test]
fn resolve_display_layer_tg_key_pressed() {
    // TG(1) pressed should display layer 1
}

#[test]
fn resolve_display_layer_lt_key_pressed() {
    // LT(2, KC_A) held should display layer 2
}

#[test]
fn resolve_display_layer_osl_key_pressed() {
    // OSL(1) pressed should display layer 1
}

#[test]
fn resolve_display_layer_multiple_layer_keys() {
    // Multiple layer keys pressed - test priority
}

#[test]
fn resolve_display_layer_v5_protocol() {
    // Test layer resolution with v5 protocol keycodes
}

#[test]
fn resolve_display_layer_v6_protocol() {
    // Test layer resolution with v6 protocol keycodes
}

#[test]
fn resolve_display_layer_momentary_release() {
    // MO key released should return to base layer
}

#[test]
fn resolve_display_layer_out_of_bounds() {
    // Layer number > available layers should clamp to valid range
}
```

#### Polling Tests
```rust
#[test]
fn polling_starts_on_test_matrix_tab() {
    // Switching to TestMatrix should start polling
}

#[test]
fn polling_stops_on_tab_change() {
    // Leaving TestMatrix should stop polling
}

#[test]
fn polling_stops_on_disconnect() {
    // Disconnect should stop polling
}

#[test]
fn polling_interval_is_30ms() {
    // Verify 30ms interval timing
}

#[test]
fn polling_handles_device_error() {
    // Device error should be handled gracefully
}

#[test]
fn polling_resumes_after_temporary_error() {
    // Should continue polling after transient error
}
```

#### Rendering Tests
```rust
#[test]
fn render_empty_key_layout() {
    // Empty key_layout should show appropriate message
}

#[test]
fn render_with_key_layout() {
    // Physical layout should render correctly
}

#[test]
fn render_key_highlight_when_pressed() {
    // Pressed keys should have highlight class
}

#[test]
fn render_key_no_highlight_when_released() {
    // Released keys should not have highlight
}

#[test]
fn render_key_label_from_layer() {
    // Key should display correct label from displayed layer
}

#[test]
fn render_layer_switch_indicator() {
    // Should indicate when viewing a different layer than base
}

#[test]
fn render_transparent_key() {
    // Transparent keys should show appropriate visual
}

#[test]
fn render_unknown_keycode() {
    // Unknown keycodes should show hex value
}

#[test]
fn render_large_keyboard() {
    // Test with 100+ key keyboard
}

#[test]
fn render_small_keyboard() {
    // Test with minimal 1x1 keyboard
}
```

### End-to-End Tests (Manual/Automated)

#### Basic Functionality
- [ ] Connect keyboard and navigate to Test Matrix tab
- [ ] Press single key - verify highlight appears
- [ ] Release single key - verify highlight disappears
- [ ] Press multiple keys simultaneously - all highlighted
- [ ] Release keys in different order - highlights removed correctly

#### Layer Switching
- [ ] Press MO(1) key - verify layer 1 displayed
- [ ] Release MO(1) key - verify returns to base layer
- [ ] Press TO(2) key - verify layer 2 displayed
- [ ] Press MO(1) + key on layer 1 - verify correct keycodes shown
- [ ] Press LT(1, KC_A) and hold - verify layer 1 displayed
- [ ] Release LT key - verify returns to base layer

#### Physical Layout
- [ ] Verify layout matches Keymap tab exactly
- [ ] Test with ANSI layout keyboard
- [ ] Test with ISO layout keyboard
- [ ] Test with ortholinear layout
- [ ] Test with split keyboard layout

#### Performance
- [ ] Verify 30ms polling doesn't cause UI lag
- [ ] Test with rapid key presses (30+ keys/second)
- [ ] Test with all keys pressed simultaneously
- [ ] Verify memory usage doesn't grow over time

#### Edge Cases
- [ ] Disconnect keyboard while on Test Matrix tab
- [ ] Reconnect keyboard while on Test Matrix tab
- [ ] Switch tabs rapidly while keys are pressed
- [ ] Browser tab background/foreground while polling
- [ ] Laptop sleep/wake while connected

#### Error Handling
- [ ] Device communication error during polling
- [ ] Invalid matrix state response from device
- [ ] Matrix size mismatch between definition and device
- [ ] Layer data unavailable but matrix polling works

### Visual Regression Tests
- [ ] Screenshot comparison: unpressed state
- [ ] Screenshot comparison: single key pressed
- [ ] Screenshot comparison: multiple keys pressed
- [ ] Screenshot comparison: layer switch active
- [ ] Dark mode rendering
- [ ] Mobile viewport rendering
- [ ] Various browser zoom levels (80%, 100%, 150%)

### Accessibility Tests
- [ ] Keyboard navigation within Test Matrix tab
- [ ] Screen reader announcements for key presses
- [ ] High contrast mode compatibility
- [ ] Color-blind friendly highlight indicator (not just color)

## Performance Considerations

1. **Polling Optimization**:
   - Use `requestAnimationFrame` scheduling to align with browser paint
   - Debounce rapid state changes
   - Skip render if matrix state unchanged

2. **Rendering Optimization**:
   - Memoize key positions
   - Use CSS transitions instead of JS animations
   - Virtual scrolling for very large keyboards

3. **Memory Management**:
   - Clean up intervals on unmount
   - Cancel in-flight requests on disconnect

## Security Considerations

1. WebHID permission is already granted for device connection
2. Matrix polling only reads state, doesn't write
3. No sensitive data exposed through matrix state

## Future Enhancements

1. **Key Logging**: Optional key press history/log
2. **Matrix Statistics**: Show key press frequency/heatmap
3. **Ghost Key Detection**: Identify phantom key presses
4. **Switch Tester Mode**: Measure actuation point consistency
5. **Comparison Mode**: Compare matrix state against expected layout
