//! Layout generation for ZMK keymaps when no physical layout is available
//!
//! Generates reasonable grid layouts based on key count and detected layout type.

use super::{layout_detector::DetectedKeyboardLayout, KeyOrigin, PhysicalKey};
use serde::{Deserialize, Serialize};

/// Information about a generated layout
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GeneratedLayoutInfo {
    pub layout_type: DetectedKeyboardLayout,
    pub confidence: f32,
    pub rows: usize,
    pub cols: usize,
}

/// Standard key widths in key units (1u = 100 units)
const STANDARD_KEY_WIDTH: i32 = 100;
const STANDARD_KEY_HEIGHT: i32 = 100;

/// Generate a square/rectangular layout based on key count and detected layout type
pub fn generate_square_layout(
    key_count: usize,
    layout_type: DetectedKeyboardLayout,
) -> (Vec<PhysicalKey>, GeneratedLayoutInfo) {
    let (rows, cols) = calculate_grid_dimensions(key_count);
    let keys = generate_grid_keys(key_count, rows, cols, layout_type);

    let info = GeneratedLayoutInfo {
        layout_type,
        confidence: 1.0,
        rows,
        cols,
    };

    (keys, info)
}

/// Calculate reasonable row/column distribution for a given key count
fn calculate_grid_dimensions(key_count: usize) -> (usize, usize) {
    match key_count {
        // Small ergo boards (3x4, 3x5, etc.)
        30..=42 => {
            let cols = key_count / 3;
            (3, cols)
        }
        // 40% / small boards (4 rows)
        43..=54 => {
            let cols = (key_count + 3) / 4;
            (4, cols)
        }
        // 60% boards (typically 4-5 rows, ~12-15 cols)
        55..=68 => {
            let cols = (key_count + 4) / 5;
            (5, cols)
        }
        // 65%/75% boards (5-6 rows)
        69..=84 => {
            let cols = (key_count + 5) / 6;
            (6, cols)
        }
        // TKL and larger (6+ rows)
        85..=100 => {
            let cols = (key_count + 5) / 6;
            (6, cols)
        }
        // Full size and very large
        101..=120 => {
            let cols = (key_count + 5) / 6;
            (6, cols)
        }
        // Very small boards (2 rows)
        20..=29 => {
            let cols = (key_count + 1) / 2;
            (2, cols)
        }
        // Tiny boards (single row or minimal)
        10..=19 => {
            let cols = (key_count + 1) / 2;
            (2, cols)
        }
        // Extremely small boards
        1..=9 => {
            let cols = key_count;
            (1, cols)
        }
        // Fallback for unusual counts - use square root based calculation
        _ => {
            let cols = (key_count as f32).sqrt().ceil() as usize;
            let rows = (key_count + cols - 1) / cols;
            (rows, cols)
        }
    }
}

/// Generate keys in a grid layout with appropriate modifier sizes
fn generate_grid_keys(
    key_count: usize,
    rows: usize,
    cols: usize,
    layout_type: DetectedKeyboardLayout,
) -> Vec<PhysicalKey> {
    let mut keys = Vec::with_capacity(key_count);
    let mut key_idx = 0;

    // Define row configurations based on layout type and typical keyboard patterns
    for row in 0..rows {
        if key_idx >= key_count {
            break;
        }

        let row_keys = calculate_row_key_count(key_count, rows, row, cols);
        let row_offset = calculate_row_offset(row, rows, row_keys, cols);

        for col in 0..row_keys {
            if key_idx >= key_count {
                break;
            }

            let (width, x_offset) = calculate_key_properties(
                row,
                col,
                row_keys,
                rows,
                layout_type,
                row_offset,
            );

            keys.push(PhysicalKey {
                x: x_offset,
                y: row as i32 * STANDARD_KEY_HEIGHT,
                width,
                height: STANDARD_KEY_HEIGHT,
                rotation: 0,
                rx: 0,
                ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            });

            key_idx += 1;
        }
    }

    keys
}

/// Calculate how many keys should be in a given row
fn calculate_row_key_count(total_keys: usize, rows: usize, row: usize, _cols: usize) -> usize {
    let base_count = total_keys / rows;
    let remainder = total_keys % rows;

    // Distribute remainder keys starting from the top rows
    if row < remainder {
        base_count + 1
    } else {
        base_count
    }
}

/// Calculate horizontal offset to center the row
fn calculate_row_offset(_row: usize, _rows: usize, row_keys: usize, cols: usize) -> i32 {
    let row_width = row_keys as i32 * STANDARD_KEY_WIDTH;
    let max_width = cols as i32 * STANDARD_KEY_WIDTH;
    (max_width - row_width) / 2
}

/// Calculate key width and x position based on position and layout type
fn calculate_key_properties(
    row: usize,
    col: usize,
    row_keys: usize,
    rows: usize,
    _layout_type: DetectedKeyboardLayout,
    row_offset: i32,
) -> (i32, i32) {
    let x = row_offset + col as i32 * STANDARD_KEY_WIDTH;

    // Apply modifier key sizing for a standard layout appearance
    let width = if rows >= 4 {
        match row {
            // Top row (numbers) - mostly 1u, with wider keys at ends
            0 => STANDARD_KEY_WIDTH,

            // QWERTY row - standard 1u keys
            1 => STANDARD_KEY_WIDTH,

            // Home row - handle special keys like Caps Lock (usually wider)
            2 => {
                if col == 0 {
                    // Caps Lock position (1.75u)
                    (STANDARD_KEY_WIDTH as f32 * 1.75) as i32
                } else {
                    STANDARD_KEY_WIDTH
                }
            }

            // Bottom alpha row - handle Shift keys
            3 => {
                if col == 0 {
                    // Left Shift (2.25u)
                    (STANDARD_KEY_WIDTH as f32 * 2.25) as i32
                } else if col == row_keys - 1 {
                    // Right Shift (2.75u)
                    (STANDARD_KEY_WIDTH as f32 * 2.75) as i32
                } else {
                    STANDARD_KEY_WIDTH
                }
            }

            // Modifiers row - various widths for Ctrl, Alt, Space, etc.
            4 => {
                if col == 0 {
                    // Left Ctrl (1.25u)
                    (STANDARD_KEY_WIDTH as f32 * 1.25) as i32
                } else if col == 1 || col == 2 {
                    // Win/Alt (1.25u each)
                    (STANDARD_KEY_WIDTH as f32 * 1.25) as i32
                } else if col == row_keys - 3 || col == row_keys - 2 {
                    // Alt/Win (1.25u each)
                    (STANDARD_KEY_WIDTH as f32 * 1.25) as i32
                } else if col == row_keys - 1 {
                    // Right Ctrl (1.25u)
                    (STANDARD_KEY_WIDTH as f32 * 1.25) as i32
                } else if col == row_keys / 2 || col == row_keys / 2 - 1 {
                    // Space bar (typically spans multiple positions)
                    // For simplicity, make it wider
                    (STANDARD_KEY_WIDTH as f32 * 1.5) as i32
                } else {
                    STANDARD_KEY_WIDTH
                }
            }

            // Any additional rows
            _ => STANDARD_KEY_WIDTH,
        }
    } else {
        // For smaller layouts (less than 4 rows), use uniform 1u keys
        STANDARD_KEY_WIDTH
    };

    (width, x)
}

/// Generate a simple rectangular grid without modifier sizing
/// Useful for ergo/split boards where uniform sizing is preferred
pub fn generate_uniform_grid(key_count: usize, cols: usize) -> Vec<PhysicalKey> {
    let _rows = (key_count + cols - 1) / cols;
    let mut keys = Vec::with_capacity(key_count);

    for i in 0..key_count {
        let row = i / cols;
        let col = i % cols;

        keys.push(PhysicalKey {
            x: col as i32 * STANDARD_KEY_WIDTH,
            y: row as i32 * STANDARD_KEY_HEIGHT,
            width: STANDARD_KEY_WIDTH,
            height: STANDARD_KEY_HEIGHT,
            rotation: 0,
            rx: 0,
            ry: 0,
            origin: KeyOrigin::Standard,
            name: String::new(),
        });
    }

    keys
}

/// Generate a split layout with a gap between halves
pub fn generate_split_layout(
    key_count: usize,
    cols_per_half: usize,
    gap_cols: usize,
) -> Vec<PhysicalKey> {
    let usable_cols = cols_per_half * 2; // Actual key columns (not including gap)
    let rows = (key_count + usable_cols - 1) / usable_cols;
    let mut keys = Vec::with_capacity(key_count);

    let mut key_idx = 0;
    for row in 0..rows {
        for col in 0..(cols_per_half + gap_cols + cols_per_half) {
            if key_idx >= key_count {
                break;
            }

            // Skip the gap columns
            if col >= cols_per_half && col < cols_per_half + gap_cols {
                continue;
            }

            let actual_x = col as i32 * STANDARD_KEY_WIDTH;

            keys.push(PhysicalKey {
                x: actual_x,
                y: row as i32 * STANDARD_KEY_HEIGHT,
                width: STANDARD_KEY_WIDTH,
                height: STANDARD_KEY_HEIGHT,
                rotation: 0,
                rx: 0,
                ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            });

            key_idx += 1;
        }
    }

    keys
}

/// Generate a fallback split layout optimized for the key count
/// This creates two "hands" with a gap in between - suitable for ergonomic keyboards
/// when no physical layout is available in the database
pub fn generate_fallback_split_layout(
    key_count: usize,
    layout_type: DetectedKeyboardLayout,
) -> (Vec<PhysicalKey>, GeneratedLayoutInfo) {
    // Determine optimal split layout dimensions based on key count
    let (rows, cols_per_half, gap_cols) = calculate_split_dimensions(key_count);
    
    // Generate the split layout
    let keys = generate_split_layout_with_info(key_count, rows, cols_per_half, gap_cols);
    
    let info = GeneratedLayoutInfo {
        layout_type,
        confidence: 1.0,
        rows,
        cols: cols_per_half * 2 + gap_cols,
    };
    
    (keys, info)
}

/// Calculate dimensions for a split layout
/// Returns (rows, cols_per_half, gap_cols)
fn calculate_split_dimensions(key_count: usize) -> (usize, usize, usize) {
    match key_count {
        // Small ergo boards (e.g., Corne 42)
        30..=44 => {
            // 3 rows, 6-7 cols per half, 2 col gap
            // With 3 rows and 6 cols per half, we can fit 36 keys
            // With 3 rows and 7 cols per half, we can fit 42 keys
            let usable_cols_per_row = 6; // 6 keys per half = 12 usable columns per row
            let rows = (key_count + usable_cols_per_row * 2 - 1) / (usable_cols_per_row * 2);
            let rows = rows.max(3).min(4);
            (rows, 6, 2)
        }
        // Medium split boards (45-60 keys like Lily58)
        45..=60 => {
            // 4 rows, 6-7 cols per half
            // With 4 rows and 6 cols per half, we can fit 48 keys
            // With 4 rows and 7 cols per half, we can fit 56 keys
            let cols_per_half = if key_count > 52 { 7 } else { 6 };
            let usable_cols_per_row = cols_per_half * 2;
            let rows = (key_count + usable_cols_per_row - 1) / usable_cols_per_row;
            let rows = rows.max(4).min(5);
            (rows, cols_per_half, 2)
        }
        // Larger split boards (61-70 keys)
        61..=70 => {
            // 4-5 rows, 7 cols per half
            let cols_per_half = 7;
            let usable_cols_per_row = cols_per_half * 2;
            let rows = (key_count + usable_cols_per_row - 1) / usable_cols_per_row;
            let rows = rows.max(4).min(5);
            (rows, cols_per_half, 2)
        }
        // Full-size split or unusual counts
        _ => {
            // Calculate based on square root, but split in half
            let target_cols = (key_count as f32).sqrt().ceil() as usize;
            let cols_per_half = (target_cols / 2).max(4);
            let gap_cols = 2;
            let usable_cols_per_row = cols_per_half * 2;
            let rows = (key_count + usable_cols_per_row - 1) / usable_cols_per_row;
            (rows, cols_per_half, gap_cols)
        }
    }
}

/// Generate a split layout with specified dimensions
fn generate_split_layout_with_info(
    key_count: usize,
    rows: usize,
    cols_per_half: usize,
    gap_cols: usize,
) -> Vec<PhysicalKey> {
    let total_cols = cols_per_half * 2 + gap_cols;
    let mut keys = Vec::with_capacity(key_count);
    let mut key_idx = 0;
    
    // Calculate starting x offset to center the layout
    let total_width = total_cols as i32 * STANDARD_KEY_WIDTH;
    let start_x = -total_width / 2;

    for row in 0..rows {
        for col in 0..total_cols {
            if key_idx >= key_count {
                break;
            }

            // Skip the gap columns
            if col >= cols_per_half && col < cols_per_half + gap_cols {
                continue;
            }

            let x = start_x + col as i32 * STANDARD_KEY_WIDTH;
            let y = row as i32 * STANDARD_KEY_HEIGHT;

            keys.push(PhysicalKey {
                x,
                y,
                width: STANDARD_KEY_WIDTH,
                height: STANDARD_KEY_HEIGHT,
                rotation: 0,
                rx: 0,
                ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            });

            key_idx += 1;
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_square_60_percent() {
        let (keys, info) = generate_square_layout(60, DetectedKeyboardLayout::Qwerty);
        assert_eq!(keys.len(), 60);
        assert_eq!(info.rows, 5);
        assert!(matches!(info.layout_type, DetectedKeyboardLayout::Qwerty));

        // Verify first key is at origin
        assert_eq!(keys[0].x, 0);
        assert_eq!(keys[0].y, 0);
        assert_eq!(keys[0].width, STANDARD_KEY_WIDTH);
    }

    #[test]
    fn test_generate_square_small_ergo() {
        let (keys, info) = generate_square_layout(36, DetectedKeyboardLayout::Qwerty);
        assert_eq!(keys.len(), 36);
        assert_eq!(info.rows, 3);

        // All keys should have standard dimensions
        for key in &keys {
            assert_eq!(key.width, STANDARD_KEY_WIDTH);
            assert_eq!(key.height, STANDARD_KEY_HEIGHT);
        }
    }

    #[test]
    fn test_generate_square_tkl() {
        let (keys, info) = generate_square_layout(87, DetectedKeyboardLayout::Qwerty);
        assert_eq!(keys.len(), 87);
        assert_eq!(info.rows, 6);
    }

    #[test]
    fn test_calculate_grid_dimensions() {
        // Small ergo boards
        assert_eq!(calculate_grid_dimensions(36), (3, 12));
        assert_eq!(calculate_grid_dimensions(42), (3, 14));

        // 60% boards
        assert_eq!(calculate_grid_dimensions(58), (5, 12));
        assert_eq!(calculate_grid_dimensions(60), (5, 12));

        // 65%/75%
        assert_eq!(calculate_grid_dimensions(68), (5, 14));
        assert_eq!(calculate_grid_dimensions(75), (6, 13));

        // TKL
        assert_eq!(calculate_grid_dimensions(87), (6, 15));

        // Very small
        assert_eq!(calculate_grid_dimensions(10), (2, 5));
        assert_eq!(calculate_grid_dimensions(5), (1, 5));
    }

    #[test]
    fn test_generate_uniform_grid() {
        let keys = generate_uniform_grid(40, 10);
        assert_eq!(keys.len(), 40);

        // Check grid structure (4 rows x 10 cols)
        assert_eq!(keys[0].x, 0);
        assert_eq!(keys[0].y, 0);
        assert_eq!(keys[10].x, 0);
        assert_eq!(keys[10].y, STANDARD_KEY_HEIGHT);
    }

    #[test]
    fn test_generate_split_layout() {
        let keys = generate_split_layout(42, 6, 1);
        assert_eq!(keys.len(), 42);

        // Check that there's a gap - keys should not be in the gap position
        // With 6 cols per half and 1 gap col, valid cols are 0-5 and 7-12
        let has_gap_key = keys.iter().any(|k| {
            let col = k.x / STANDARD_KEY_WIDTH;
            col == 6 // This would be in the gap
        });
        assert!(!has_gap_key, "Should not have keys in the gap column");
    }

    #[test]
    fn test_generated_layout_info() {
        let (_, info) = generate_square_layout(60, DetectedKeyboardLayout::Colemak);
        assert!(matches!(info.layout_type, DetectedKeyboardLayout::Colemak));
        assert_eq!(info.confidence, 1.0);
        assert_eq!(info.rows, 5);
        assert_eq!(info.cols, 12);
    }

    #[test]
    fn test_keys_have_correct_origin() {
        let (keys, _) = generate_square_layout(40, DetectedKeyboardLayout::Qwerty);
        for key in &keys {
            assert!(matches!(key.origin, KeyOrigin::Standard));
        }
    }

    #[test]
    fn test_large_layout() {
        let (keys, info) = generate_square_layout(104, DetectedKeyboardLayout::Qwerty);
        assert_eq!(keys.len(), 104);
        assert_eq!(info.rows, 6);
    }

    #[test]
    fn test_single_key() {
        let (keys, info) = generate_square_layout(1, DetectedKeyboardLayout::Unknown);
        assert_eq!(keys.len(), 1);
        assert_eq!(info.rows, 1);
        assert_eq!(info.cols, 1);
        assert_eq!(keys[0].x, 0);
        assert_eq!(keys[0].y, 0);
    }

    // ============================================================================
    // Split Layout Tests
    // ============================================================================

    #[test]
    fn test_generate_fallback_split_layout_small() {
        let (keys, info) = generate_fallback_split_layout(42, DetectedKeyboardLayout::Qwerty);
        assert_eq!(keys.len(), 42);
        // Rows can be 3 or 4 depending on the calculated dimensions
        assert!(info.rows >= 3 && info.rows <= 4, "Expected 3-4 rows for 42 keys, got {}", info.rows);
        
        // Check that there's a gap between the two halves
        let left_half_max_x = keys.iter()
            .filter(|k| k.x < 0)  // Left half has negative x due to centering
            .map(|k| k.x)
            .max()
            .unwrap();
        
        let right_half_min_x = keys.iter()
            .filter(|k| k.x > 0)  // Right half has positive x
            .map(|k| k.x)
            .min()
            .unwrap();
        
        // There should be a gap between left and right halves
        assert!(right_half_min_x - left_half_max_x > STANDARD_KEY_WIDTH * 2);
    }

    #[test]
    fn test_generate_fallback_split_layout_medium() {
        let (keys, info) = generate_fallback_split_layout(58, DetectedKeyboardLayout::Colemak);
        assert_eq!(keys.len(), 58);
        // Rows can be 4 or 5 depending on the calculated dimensions
        assert!(info.rows >= 4 && info.rows <= 5, "Expected 4-5 rows for 58 keys, got {}", info.rows);
        
        // All keys should have standard dimensions (uniform for split)
        for key in &keys {
            assert_eq!(key.width, STANDARD_KEY_WIDTH);
            assert_eq!(key.height, STANDARD_KEY_HEIGHT);
        }
    }

    #[test]
    fn test_calculate_split_dimensions() {
        // Small board (42 keys like Corne)
        let (rows, cols_per_half, gap) = calculate_split_dimensions(42);
        assert!(rows >= 3 && rows <= 4, "Expected 3-4 rows for 42 keys, got {}", rows);
        assert!(cols_per_half >= 5 && cols_per_half <= 7);
        assert_eq!(gap, 2);

        // Medium board (58 keys like Lily58)
        let (rows, cols_per_half, gap) = calculate_split_dimensions(58);
        assert!(rows >= 4 && rows <= 5, "Expected 4-5 rows for 58 keys, got {}", rows);
        assert!(cols_per_half >= 5 && cols_per_half <= 7);
        assert_eq!(gap, 2);

        // Large board (70 keys)
        let (rows, cols_per_half, gap) = calculate_split_dimensions(70);
        assert!(rows >= 4 && rows <= 5, "Expected 4-5 rows for 70 keys, got {}", rows);
        assert!(cols_per_half >= 6 && cols_per_half <= 7);
        assert_eq!(gap, 2);
    }

    #[test]
    fn test_split_layout_has_gap() {
        let keys = generate_split_layout(42, 6, 2);
        assert_eq!(keys.len(), 42);

        // With 6 cols per half and 2 gap cols, valid cols are 0-5 and 8-13
        // Check that no key is in the gap position (cols 6-7)
        for key in &keys {
            // Calculate col from x position
            let col = key.x / STANDARD_KEY_WIDTH;
            // Gap is between cols 6-7 (0-indexed: cols 6 and 7 are the 2-col gap)
            assert!(!((col >= 6 && col < 8)), "Key should not be in gap column at col {}", col);
        }
    }

    #[test]
    fn test_fallback_split_layout_is_centered() {
        let (keys, _) = generate_fallback_split_layout(42, DetectedKeyboardLayout::Qwerty);
        
        // Layout should be centered around x=0
        let min_x = keys.iter().map(|k| k.x).min().unwrap();
        let max_x = keys.iter().map(|k| k.x).max().unwrap();
        
        // The center point should be roughly at 0
        let center = (min_x + max_x + STANDARD_KEY_WIDTH) / 2;
        assert!(center.abs() < STANDARD_KEY_WIDTH);
    }
}
