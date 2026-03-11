use crate::keymap::PhysicalKey;
use std::collections::{BTreeMap, HashMap};
use lazy_static::lazy_static;

lazy_static! {
    static ref STANDARD_108_LAYOUT: HashMap<&'static str, (i32, i32)> = {
        let mut m = HashMap::new();
        // Row 0: Function row
        m.insert("esc", (0, 0));
        m.insert("f1", (2000, 0));
        m.insert("f2", (3000, 0));
        m.insert("f3", (4000, 0));
        m.insert("f4", (5000, 0));
        m.insert("f5", (6500, 0));
        m.insert("f6", (7500, 0));
        m.insert("f7", (8500, 0));
        m.insert("f8", (9500, 0));
        m.insert("f9", (11000, 0));
        m.insert("f10", (12000, 0));
        m.insert("f11", (13000, 0));
        m.insert("f12", (14000, 0));
        m.insert("prnt", (15250, 0));
        m.insert("scrl", (16250, 0));
        m.insert("paus", (17250, 0));
        
        // Extra 4 keys above numpad
        m.insert("calc", (19000, 0));
        m.insert("mail", (20000, 0));
        m.insert("vold", (21000, 0));
        m.insert("volu", (22000, 0));

        // Row 1: Number row
        let r1_y = 1200;
        m.insert("grv", (0, r1_y));
        m.insert("1", (1000, r1_y));
        m.insert("2", (2000, r1_y));
        m.insert("3", (3000, r1_y));
        m.insert("4", (4000, r1_y));
        m.insert("5", (5000, r1_y));
        m.insert("6", (6000, r1_y));
        m.insert("7", (7000, r1_y));
        m.insert("8", (8000, r1_y));
        m.insert("9", (9000, r1_y));
        m.insert("0", (10000, r1_y));
        m.insert("min", (11000, r1_y));
        m.insert("eql", (12000, r1_y));
        m.insert("bspc", (13000, r1_y));
        
        m.insert("ins", (15250, r1_y));
        m.insert("home", (16250, r1_y));
        m.insert("pgup", (17250, r1_y));
        
        m.insert("nlck", (19000, r1_y));
        m.insert("kp/", (20000, r1_y));
        m.insert("kp*", (21000, r1_y));
        m.insert("kp-", (22000, r1_y));

        // Row 2: QWERTY
        let r2_y = 2200;
        m.insert("tab", (0, r2_y));
        m.insert("q", (1500, r2_y));
        m.insert("w", (2500, r2_y));
        m.insert("e", (3500, r2_y));
        m.insert("r", (4500, r2_y));
        m.insert("t", (5500, r2_y));
        m.insert("y", (6500, r2_y));
        m.insert("u", (7500, r2_y));
        m.insert("i", (8500, r2_y));
        m.insert("o", (9500, r2_y));
        m.insert("p", (10500, r2_y));
        m.insert("lbkt", (11500, r2_y));
        m.insert("rbkt", (12500, r2_y));
        m.insert("bksl", (13500, r2_y));
        
        m.insert("del", (15250, r2_y));
        m.insert("end", (16250, r2_y));
        m.insert("pgdn", (17250, r2_y));
        
        m.insert("kp7", (19000, r2_y));
        m.insert("kp8", (20000, r2_y));
        m.insert("kp9", (21000, r2_y));
        m.insert("kp+", (22000, r2_y));

        // Row 3: ASDF
        let r3_y = 3200;
        m.insert("caps", (0, r3_y));
        m.insert("a", (1750, r3_y));
        m.insert("s", (2750, r3_y));
        m.insert("d", (3750, r3_y));
        m.insert("f", (4750, r3_y));
        m.insert("g", (5750, r3_y));
        m.insert("h", (6750, r3_y));
        m.insert("j", (7750, r3_y));
        m.insert("k", (8750, r3_y));
        m.insert("l", (9750, r3_y));
        m.insert("scln", (10750, r3_y));
        m.insert("apos", (11750, r3_y));
        m.insert("ent", (12750, r3_y));
        m.insert("ret", (12750, r3_y));
        
        m.insert("kp4", (19000, r3_y));
        m.insert("kp5", (20000, r3_y));
        m.insert("kp6", (21000, r3_y));

        // Row 4: ZXCV
        let r4_y = 4200;
        m.insert("lsft", (0, r4_y));
        m.insert("z", (2250, r4_y));
        m.insert("x", (3250, r4_y));
        m.insert("c", (4250, r4_y));
        m.insert("v", (5250, r4_y));
        m.insert("b", (6250, r4_y));
        m.insert("n", (7250, r4_y));
        m.insert("m", (8250, r4_y));
        m.insert("comm", (9250, r4_y));
        m.insert("dot", (10250, r4_y));
        m.insert("slsh", (11250, r4_y));
        m.insert("rsft", (12250, r4_y));
        
        m.insert("up", (16250, r4_y));
        
        m.insert("kp1", (19000, r4_y));
        m.insert("kp2", (20000, r4_y));
        m.insert("kp3", (21000, r4_y));
        m.insert("kpent", (22000, r4_y));

        // Row 5: Bottom
        let r5_y = 5200;
        m.insert("lctl", (0, r5_y));
        m.insert("lmet", (1250, r5_y));
        m.insert("lalt", (2500, r5_y));
        m.insert("spc", (3750, r5_y));
        m.insert("ralt", (9000, r5_y));
        m.insert("rmet", (10250, r5_y));
        m.insert("comp", (11500, r5_y));
        m.insert("rctl", (12750, r5_y));
        
        m.insert("left", (15250, r5_y));
        m.insert("down", (16250, r5_y));
        m.insert("right", (17250, r5_y));
        
        m.insert("kp0", (19000, r5_y));
        m.insert("kp.", (21000, r5_y));
        
        // Aliases for common punctuation
        m.insert(".", *m.get("dot").unwrap());
        m.insert(",", *m.get("comm").unwrap());
        m.insert("/", *m.get("slsh").unwrap());
        m.insert(";", *m.get("scln").unwrap());
        m.insert("'", *m.get("apos").unwrap());
        m.insert("[", *m.get("lbkt").unwrap());
        m.insert("]", *m.get("rbkt").unwrap());
        m.insert("\\", *m.get("bksl").unwrap());
        m.insert("-", *m.get("min").unwrap());
        m.insert("=", *m.get("eql").unwrap());
        m.insert("`", *m.get("grv").unwrap());
        
        m
    };

    static ref MAC_LAYOUT: HashMap<&'static str, (i32, i32)> = {
        let mut m = STANDARD_108_LAYOUT.clone();
        let r5_y = 5200;
        // Mac bottom row: Ctrl, Opt, Cmd, Space, Cmd, Opt, Ctrl
        m.insert("lctl", (0, r5_y));
        m.insert("lalt", (1250, r5_y));
        m.insert("lmet", (2500, r5_y));
        m.insert("spc", (3750, r5_y));
        m.insert("rmet", (9000, r5_y));
        m.insert("ralt", (10250, r5_y));
        m.insert("rctl", (11500, r5_y));
        // Remove comp/rmet if they overlap or are different
        m.remove("comp"); 
        m
    };

    static ref WIN_LAPTOP_LAYOUT: HashMap<&'static str, (i32, i32)> = {
        let mut m = HashMap::new();
        // Row 0: Function row
        m.insert("esc", (0, 0));
        m.insert("f1", (1500, 0));
        m.insert("f2", (2500, 0));
        m.insert("f3", (3500, 0));
        m.insert("f4", (4500, 0));
        m.insert("f5", (5500, 0));
        m.insert("f6", (6500, 0));
        m.insert("f7", (7500, 0));
        m.insert("f8", (8500, 0));
        m.insert("f9", (9500, 0));
        m.insert("f10", (10500, 0));
        m.insert("f11", (11500, 0));
        m.insert("f12", (12500, 0));
        m.insert("ins", (13750, 0));
        m.insert("del", (14750, 0));

        // Row 1: Number row
        let r1_y = 1000;
        m.insert("grv", (0, r1_y));
        m.insert("1", (1000, r1_y));
        m.insert("2", (2000, r1_y));
        m.insert("3", (3000, r1_y));
        m.insert("4", (4000, r1_y));
        m.insert("5", (5000, r1_y));
        m.insert("6", (6000, r1_y));
        m.insert("7", (7000, r1_y));
        m.insert("8", (8000, r1_y));
        m.insert("9", (9000, r1_y));
        m.insert("0", (10000, r1_y));
        m.insert("min", (11000, r1_y));
        m.insert("eql", (12000, r1_y));
        m.insert("bspc", (13000, r1_y));
        m.insert("home", (14500, r1_y));

        // Row 2: QWERTY
        let r2_y = 2000;
        m.insert("tab", (0, r2_y));
        m.insert("q", (1250, r2_y));
        m.insert("w", (2250, r2_y));
        m.insert("e", (3250, r2_y));
        m.insert("r", (4250, r2_y));
        m.insert("t", (5250, r2_y));
        m.insert("y", (6250, r2_y));
        m.insert("u", (7250, r2_y));
        m.insert("i", (8250, r2_y));
        m.insert("o", (9250, r2_y));
        m.insert("p", (10250, r2_y));
        m.insert("lbkt", (11250, r2_y));
        m.insert("rbkt", (12250, r2_y));
        m.insert("bksl", (13250, r2_y));
        m.insert("pgup", (14500, r2_y));

        // Row 3: ASDF
        let r3_y = 3000;
        m.insert("caps", (0, r3_y));
        m.insert("a", (1500, r3_y));
        m.insert("s", (2500, r3_y));
        m.insert("d", (3500, r3_y));
        m.insert("f", (4500, r3_y));
        m.insert("g", (5500, r3_y));
        m.insert("h", (6500, r3_y));
        m.insert("j", (7500, r3_y));
        m.insert("k", (8500, r3_y));
        m.insert("l", (9500, r3_y));
        m.insert("scln", (10500, r3_y));
        m.insert("apos", (11500, r3_y));
        m.insert("ent", (12500, r3_y));
        m.insert("pgdn", (14500, r3_y));

        // Row 4: ZXCV
        let r4_y = 4000;
        m.insert("lsft", (0, r4_y));
        m.insert("z", (2000, r4_y));
        m.insert("x", (3000, r4_y));
        m.insert("c", (4000, r4_y));
        m.insert("v", (5000, r4_y));
        m.insert("b", (6000, r4_y));
        m.insert("n", (7000, r4_y));
        m.insert("m", (8000, r4_y));
        m.insert("comm", (9000, r4_y));
        m.insert("dot", (10000, r4_y));
        m.insert("slsh", (11000, r4_y));
        m.insert("rsft", (12000, r4_y));
        m.insert("up", (13750, r4_y));
        m.insert("end", (14750, r4_y));

        // Row 5: Bottom
        let r5_y = 5000;
        m.insert("lctl", (0, r5_y));
        m.insert("lmet", (1000, r5_y));
        m.insert("lalt", (2000, r5_y));
        m.insert("spc", (3000, r5_y));
        m.insert("ralt", (8000, r5_y));
        m.insert("rctl", (9000, r5_y));
        m.insert("left", (12750, r5_y));
        m.insert("down", (13750, r5_y));
        m.insert("right", (14750, r5_y));

        // Punctuation aliases
        m.insert(".", (10000, r4_y));
        m.insert(",", (9000, r4_y));
        m.insert("/", (11000, r4_y));
        m.insert(";", (10500, r3_y));
        m.insert("'", (11500, r3_y));
        m.insert("[", (11250, r2_y));
        m.insert("]", (12250, r2_y));
        m.insert("\\", (13250, r2_y));
        m.insert("-", (11000, r1_y));
        m.insert("=", (12000, r1_y));
        m.insert("`", (0, r1_y));

        m
    };

    static ref MACBOOK_LAYOUT: HashMap<&'static str, (i32, i32)> = {
        let mut m = HashMap::new();
        // Row 0: Function row (Mac style: Escape then F1-F12, then TouchID/Power)
        m.insert("esc", (0, 0));
        m.insert("f1", (1500, 0));
        m.insert("f2", (2500, 0));
        m.insert("f3", (3500, 0));
        m.insert("f4", (4500, 0));
        m.insert("f5", (5500, 0));
        m.insert("f6", (6500, 0));
        m.insert("f7", (7500, 0));
        m.insert("f8", (8500, 0));
        m.insert("f9", (9500, 0));
        m.insert("f10", (10500, 0));
        m.insert("f11", (11500, 0));
        m.insert("f12", (12500, 0));

        // Row 1: Number row
        let r1_y = 1000;
        m.insert("grv", (0, r1_y));
        m.insert("1", (1000, r1_y));
        m.insert("2", (2000, r1_y));
        m.insert("3", (3000, r1_y));
        m.insert("4", (4000, r1_y));
        m.insert("5", (5000, r1_y));
        m.insert("6", (6000, r1_y));
        m.insert("7", (7000, r1_y));
        m.insert("8", (8000, r1_y));
        m.insert("9", (9000, r1_y));
        m.insert("0", (10000, r1_y));
        m.insert("min", (11000, r1_y));
        m.insert("eql", (12000, r1_y));
        m.insert("bspc", (13000, r1_y));

        // Row 2: QWERTY
        let r2_y = 2000;
        m.insert("tab", (0, r2_y));
        m.insert("q", (1250, r2_y));
        m.insert("w", (2250, r2_y));
        m.insert("e", (3250, r2_y));
        m.insert("r", (4250, r2_y));
        m.insert("t", (5250, r2_y));
        m.insert("y", (6250, r2_y));
        m.insert("u", (7250, r2_y));
        m.insert("i", (8250, r2_y));
        m.insert("o", (9250, r2_y));
        m.insert("p", (10250, r2_y));
        m.insert("lbkt", (11250, r2_y));
        m.insert("rbkt", (12250, r2_y));
        m.insert("bksl", (13250, r2_y));

        // Row 3: ASDF
        let r3_y = 3000;
        m.insert("caps", (0, r3_y));
        m.insert("a", (1500, r3_y));
        m.insert("s", (2500, r3_y));
        m.insert("d", (3500, r3_y));
        m.insert("f", (4500, r3_y));
        m.insert("g", (5500, r3_y));
        m.insert("h", (6500, r3_y));
        m.insert("j", (7500, r3_y));
        m.insert("k", (8500, r3_y));
        m.insert("l", (9500, r3_y));
        m.insert("scln", (10500, r3_y));
        m.insert("apos", (11500, r3_y));
        m.insert("ent", (12500, r3_y));

        // Row 4: ZXCV
        let r4_y = 4000;
        m.insert("lsft", (0, r4_y));
        m.insert("z", (2000, r4_y));
        m.insert("x", (3000, r4_y));
        m.insert("c", (4000, r4_y));
        m.insert("v", (5000, r4_y));
        m.insert("b", (6000, r4_y));
        m.insert("n", (7000, r4_y));
        m.insert("m", (8000, r4_y));
        m.insert("comm", (9000, r4_y));
        m.insert("dot", (10000, r4_y));
        m.insert("slsh", (11000, r4_y));
        m.insert("rsft", (12000, r4_y));
        m.insert("up", (14250, r4_y));

        // Row 5: Bottom
        let r5_y = 5000;
        m.insert("fn", (0, r5_y));
        m.insert("lctl", (1000, r5_y));
        m.insert("lalt", (2000, r5_y));
        m.insert("lmet", (3000, r5_y));
        m.insert("spc", (4250, r5_y));
        m.insert("rmet", (9500, r5_y));
        m.insert("ralt", (10500, r5_y));
        m.insert("left", (13250, r5_y));
        m.insert("down", (14250, r5_y));
        m.insert("right", (15250, r5_y));

        // Punctuation aliases
        m.insert(".", (10000, r4_y));
        m.insert(",", (9000, r4_y));
        m.insert("/", (11000, r4_y));
        m.insert(";", (10500, r3_y));
        m.insert("'", (11500, r3_y));
        m.insert("[", (11250, r2_y));
        m.insert("]", (12250, r2_y));
        m.insert("\\", (13250, r2_y));
        m.insert("-", (11000, r1_y));
        m.insert("=", (12000, r1_y));
        m.insert("`", (0, r1_y));

        m
    };
}

/// Checks if a key name is part of the standard layout.
pub fn is_standard_key(name: &str, is_mac: bool, is_laptop: bool) -> bool {
    let layout = match (is_mac, is_laptop) {
        (true, true) => &*MACBOOK_LAYOUT,
        (true, false) => &*MAC_LAYOUT,
        (false, true) => &*WIN_LAPTOP_LAYOUT,
        (false, false) => &*STANDARD_108_LAYOUT,
    };
    layout.contains_key(name.to_lowercase().as_str())
}

/// Computes physical layout using standard keyboard positions.
/// Only keys present in standard 108-key layout are shown.
/// Aliases are added at the bottom as special keys.
pub fn compute_standard_kanata_layout(key_names: &[String], unmapped_names: &[String], alias_names: &[String], is_mac: bool, is_laptop: bool) -> Vec<PhysicalKey> {
    let mut layout = Vec::new();
    let key_width = 1000;
    let key_height = 1000;
    let physical_layout = match (is_mac, is_laptop) {
        (true, true) => &*MACBOOK_LAYOUT,
        (true, false) => &*MAC_LAYOUT,
        (false, true) => &*WIN_LAPTOP_LAYOUT,
        (false, false) => &*STANDARD_108_LAYOUT,
    };

    // 1. Process standard keys from defsrc
    for name in key_names {
        if let Some(&(x, y)) = physical_layout.get(name.to_lowercase().as_str()) {
            layout.push(PhysicalKey {
                x,
                y,
                width: key_width,
                height: key_height,
                rotation: 0,
                rx: 0,
                ry: 0,
            });
        }
    }

    // 2. Process unmapped keys
    let unmapped_y_start = 6500;
    let unmapped_margin = 100;
    let unmapped_cols = 10;

    for (i, _name) in unmapped_names.iter().enumerate() {
        let col = i % unmapped_cols;
        let row = i / unmapped_cols;
        layout.push(PhysicalKey {
            x: col as i32 * (key_width + unmapped_margin),
            y: unmapped_y_start + row as i32 * (key_height + unmapped_margin),
            width: key_width,
            height: key_height,
            rotation: 0,
            rx: 0,
            ry: 0,
        });
    }

    // 3. Process aliases at the bottom
    let alias_y_start = 8000;
    let alias_margin = 100;
    let alias_cols = 10;
    
    for (i, _name) in alias_names.iter().enumerate() {
        let col = i % alias_cols;
        let row = i / alias_cols;
        
        layout.push(PhysicalKey {
            x: col as i32 * (key_width + alias_margin),
            y: alias_y_start + row as i32 * (key_height + alias_margin),
            width: key_width,
            height: key_height,
            rotation: 0,
            rx: 0,
            ry: 0,
        });
    }

    layout
}

/// Computes a compact physical layout from (row, column) positions found in a file.
pub fn compute_compact_kanata_layout(key_positions: &[(usize, usize)]) -> Vec<PhysicalKey> {
    if key_positions.is_empty() {
        return Vec::new();
    }

    // Group columns by row index. We use a BTreeMap to keep rows sorted by file row number.
    let mut rows: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(row, col) in key_positions {
        rows.entry(row).or_default().push(col);
    }

    // Map each (file_row, file_col) to its new (x, y) physical coordinate.
    let mut pos_to_coord = std::collections::HashMap::new();

    // Standard key size in units.
    let key_width = 1000;
    let key_height = 1000;
    
    // Spacing: key size + a small margin (e.g., 40 units = 2 pixels at 0.05 scale).
    let margin = 40;
    let horizontal_spacing = key_width + margin;
    let vertical_spacing = key_height + margin;

    // Use row_idx (0, 1, 2...) instead of the actual file_row to ensure rows are tight.
    for (row_idx, (file_row, mut cols)) in rows.into_iter().enumerate() {
        // Sort columns in this row to maintain their relative left-to-right order.
        cols.sort_unstable();
        
        for (col_idx, file_col) in cols.into_iter().enumerate() {
            let x = col_idx as i32 * horizontal_spacing;
            let y = row_idx as i32 * vertical_spacing;
            pos_to_coord.insert((file_row, file_col), (x, y));
        }
    }

    // Construct the PhysicalKey objects in the same order as the input positions.
    key_positions
        .iter()
        .map(|pos| {
            let (x, y) = pos_to_coord.get(pos).cloned().expect("Position must exist in map");
            PhysicalKey {
                x,
                y,
                width: key_width,
                height: key_height,
                rotation: 0,
                rx: 0,
                ry: 0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_layout() {
        // Simple 2x2 grid in file, with different column alignments
        let positions = vec![
            (10, 2), (10, 10), // Row 10: 'a' at col 2, 'b' at col 10
            (11, 3), (11, 11), // Row 11: 'c' at col 3, 'd' at col 11
        ];
        let layout = compute_compact_kanata_layout(&positions);
        
        assert_eq!(layout.len(), 4);
        
        // Key 'a' (10, 2) should be at (0, 0)
        assert_eq!(layout[0].x, 0);
        assert_eq!(layout[0].y, 0);
        
        // Key 'b' (10, 10) should be at (1040, 0)
        assert_eq!(layout[1].x, 1040);
        assert_eq!(layout[1].y, 0);
        
        // Key 'c' (11, 3) should be at (0, 1040)
        assert_eq!(layout[2].x, 0);
        assert_eq!(layout[2].y, 1040);
        
        // Key 'd' (11, 11) should be at (1040, 1040)
        assert_eq!(layout[3].x, 1040);
        assert_eq!(layout[3].y, 1040);
    }

    #[test]
    fn test_diagonal_input_to_column_output() {
        // Staggered input (e.g., diagonal in file)
        let positions = vec![(1, 2), (2, 4), (3, 6)];
        let layout = compute_compact_kanata_layout(&positions);
        
        // Each key is the only one in its row, so they all should have x=0
        assert_eq!(layout[0].x, 0);
        assert_eq!(layout[0].y, 0);
        
        assert_eq!(layout[1].x, 0);
        assert_eq!(layout[1].y, 1040);
        
        assert_eq!(layout[2].x, 0);
        assert_eq!(layout[2].y, 2080);
    }

    #[test]
    fn test_kanata_surf_like_layout() {
        // Sample positions like in kanata-surf.kbd defsrc
        // esc(10,2) f1(10,7) f2(10,12)
        // grv(11,2) 1(11,7)  2(11,12)
        let positions = vec![
            (10, 2), (10, 7), (10, 12),
            (11, 2), (11, 7), (11, 12),
        ];
        let layout = compute_compact_kanata_layout(&positions);
        
        assert_eq!(layout.len(), 6);
        
        // First row
        assert_eq!(layout[0].x, 0);    assert_eq!(layout[0].y, 0);
        assert_eq!(layout[1].x, 1040); assert_eq!(layout[1].y, 0);
        assert_eq!(layout[2].x, 2080); assert_eq!(layout[2].y, 0);
        
        // Second row
        assert_eq!(layout[3].x, 0);    assert_eq!(layout[3].y, 1040);
        assert_eq!(layout[4].x, 1040); assert_eq!(layout[4].y, 1040);
        assert_eq!(layout[5].x, 2080); assert_eq!(layout[5].y, 1040);
    }
    
    #[test]
    fn test_empty_input() {
        let layout = compute_compact_kanata_layout(&[]);
        assert!(layout.is_empty());
    }

    #[test]
    fn test_standard_layout() {
        let key_names = vec!["esc".to_string(), "f1".to_string(), "a".to_string(), "invalid".to_string()];
        let alias_names = vec!["alias1".to_string(), "alias2".to_string()];
        
        let layout = compute_standard_kanata_layout(&key_names, &[], &alias_names, false, false);
        
        // "esc", "f1", "a" are standard. "invalid" is not. 2 aliases. Total = 3 + 2 = 5
        assert_eq!(layout.len(), 5);
        
        // esc at (0, 0)
        assert_eq!(layout[0].x, 0);
        assert_eq!(layout[0].y, 0);
        
        // f1 at (2000, 0)
        assert_eq!(layout[1].x, 2000);
        assert_eq!(layout[1].y, 0);
        
        // a at (1750, 3200)
        assert_eq!(layout[2].x, 1750);
        assert_eq!(layout[2].y, 3200);
        
        // alias1 at (0, 8000) - aliases start at alias_y_start = 8000
        assert_eq!(layout[3].x, 0);
        assert_eq!(layout[3].y, 8000);
        
        // alias2 at (1100, 8000)
        assert_eq!(layout[4].x, 1100);
        assert_eq!(layout[4].y, 8000);
    }

    #[test]
    fn test_is_standard_key() {
        assert!(is_standard_key("esc", false, false));
        assert!(is_standard_key("ESC", false, false));
        assert!(is_standard_key("A", false, false));
        assert!(is_standard_key("kp0", false, false));
        assert!(!is_standard_key("invalid", false, false));
    }
}
