//! Fetch and convert keyboard layouts from nickcoutsos/keymap-editor-contrib
//!
//! This binary fetches keyboard JSON files from the contrib repository
//! and converts them to our ZmkLayout format.
//!
//! Usage:
//!   # With runfiles (Bazel build):
//!   bazel run //server:fetch_contrib_layouts
//!
//!   # With explicit path:
//!   bazel run //server:fetch_contrib_layouts -- <keyboards_dir> [output_path]
//!
//! To download the keyboards manually:
//!   git clone --depth 1 https://github.com/nickcoutsos/keymap-editor-contrib.git
//!   bazel run //server:fetch_contrib_layouts -- keymap-editor-contrib/keyboards

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Nickcoutsos format for a key in the layout
#[derive(Debug, Clone, Deserialize)]
struct ContribKey {
    row: i32,
    col: i32,
    #[serde(default)]
    r: Option<f32>,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    rx: Option<f32>,
    #[serde(default)]
    ry: Option<f32>,
    #[serde(default)]
    w: Option<f32>,
    #[serde(default)]
    h: Option<f32>,
}

/// Nickcoutsos format for a layout (array format)
#[derive(Debug, Clone, Deserialize)]
struct ContribLayoutItem {
    name: String,
    layout: Vec<ContribKey>,
}

/// Nickcoutsos format for a layout (map format - legacy)
#[derive(Debug, Clone, Deserialize)]
struct ContribLayoutMap {
    layout: Vec<ContribKey>,
}

/// Nickcoutsos format for a keyboard
/// Supports both array format (new) and map format (legacy)
#[derive(Debug, Clone, Deserialize)]
struct ContribKeyboard {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    // Use untagged enum to support both formats
    #[serde(default, deserialize_with = "deserialize_layouts")]
    layouts: Vec<ContribLayoutItem>,
}

fn deserialize_layouts<'de, D>(deserializer: D) -> Result<Vec<ContribLayoutItem>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, MapAccess, SeqAccess, Visitor};
    use std::fmt;

    struct LayoutsVisitor;

    impl<'de> Visitor<'de> for LayoutsVisitor {
        type Value = Vec<ContribLayoutItem>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a map or array of layouts")
        }

        // Handle array format: [{ "name": "...", "layout": [...] }]
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut layouts = Vec::new();
            while let Some(item) = seq.next_element::<ContribLayoutItem>()? {
                layouts.push(item);
            }
            Ok(layouts)
        }

        // Handle map format: { "default_transform": { "layout": [...] } }
        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut layouts = Vec::new();
            while let Some((name, layout_map)) = map.next_entry::<String, ContribLayoutMap>()? {
                layouts.push(ContribLayoutItem {
                    name,
                    layout: layout_map.layout,
                });
            }
            Ok(layouts)
        }
    }

    deserializer.deserialize_any(LayoutsVisitor)
}

/// Our internal representation for conversion
#[derive(Debug, Clone)]
struct PhysicalKey {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
    pub rotation: i32,
    pub rx: i32,
    pub ry: i32,
}

#[derive(Debug, Clone)]
struct Layout {
    pub name: String,
    pub display_name: Option<String>,
    pub keys: Vec<PhysicalKey>,
    pub source_file: String,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let input_path: std::path::PathBuf;
    let output_path: Option<std::path::PathBuf>;

    if args.len() >= 2 {
        // Explicit path provided - use it directly
        input_path = std::path::PathBuf::from(&args[1]);
        output_path = if args.len() >= 3 {
            Some(std::path::PathBuf::from(&args[2]))
        } else if let Some(ws_root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
            let mut p = std::path::PathBuf::from(ws_root);
            p.push("src/keymap/contrib_layouts.rs");
            Some(p)
        } else {
            Some(std::path::PathBuf::from("src/keymap/contrib_layouts.rs"))
        };
    } else {
        // No arguments - use runfiles to find the keyboard data
        #[cfg(feature = "runfiles")]
        {
            let r = runfiles::Runfiles::create()?;
            // The MANIFEST only lists files, not directories, so we need to resolve
            // a known file and get its parent directory
            let sample_file = runfiles::rlocation!(
                r,
                "keymap_editor_contrib/keyboard-data/a_dux.json"
            )
            .context("Failed to find keyboard data in runfiles. Make sure @keymap_editor_contrib//:keyboard_data is included as a data dependency.")?;
            
            input_path = sample_file
                .parent()
                .map(|p| p.to_path_buf())
                .context("Could not get keyboard-data directory from sample file path")?;
                
            output_path = if let Some(ws_root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
                let mut p = std::path::PathBuf::from(ws_root);
                p.push("src/keymap/contrib_layouts.rs");
                Some(p)
            } else {
                Some(std::path::PathBuf::from("src/keymap/contrib_layouts.rs"))
            };
        }
        #[cfg(not(feature = "runfiles"))]
        {
            eprintln!("Usage: {} <keyboards_dir> [output_path]", args[0]);
            eprintln!("");
            eprintln!("Downloads or reads keyboard layouts from nickcoutsos/keymap-editor-contrib");
            eprintln!("and converts them to the ZmkLayout format.");
            eprintln!("");
            eprintln!("This binary requires the 'runfiles' feature to be enabled when built with Bazel,");
            eprintln!("or you can manually provide the keyboards directory.");
            eprintln!("");
            eprintln!("To get the keyboards directory:");
            eprintln!("  git clone --depth 1 https://github.com/nickcoutsos/keymap-editor-contrib.git");
            eprintln!("  {} keymap-editor-contrib/keyboards", args[0]);
            std::process::exit(1);
        }
    };

    let mut layouts = Vec::new();

    // Process all JSON files in the input directory
    if input_path.is_dir() {
        process_directory(&input_path, &mut layouts)?;
    } else if input_path.is_file() {
        // Single file mode for testing
        if let Some(layout) = process_file(&input_path)? {
            layouts.push(layout);
        }
    } else {
        anyhow::bail!("Input path does not exist: {:?}", input_path);
    }

    // Generate output
    let output = generate_rust_code(&layouts);

    if let Some(out_path) = output_path {
        fs::write(&out_path, output)?;
        println!("Generated {} layouts to {:?}", layouts.len(), out_path);
    } else {
        println!("{}", output);
    }

    Ok(())
}

fn process_directory(dir: &Path, layouts: &mut Vec<Layout>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            process_directory(&path, layouts)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(layout) = process_file(&path)? {
                layouts.push(layout);
            }
        }
    }
    Ok(())
}

fn process_file(path: &Path) -> Result<Option<Layout>> {
    let content = fs::read_to_string(path)?;
    let keyboard: ContribKeyboard = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {:?}", path))?;

    // Use the "default_transform" layout if available, otherwise use the first one
    let contrib_layout = keyboard
        .layouts
        .iter()
        .find(|l| l.name == "default_transform")
        .or_else(|| keyboard.layouts.first())
        .context("No layouts found in keyboard file")?;

    // Convert keys
    let keys: Vec<PhysicalKey> = contrib_layout
        .layout
        .iter()
        .map(|k| convert_key(k))
        .collect();

    if keys.is_empty() {
        return Ok(None);
    }

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Some(Layout {
        name: keyboard.id,
        display_name: Some(keyboard.name),
        keys,
        source_file: format!("nickcoutsos/keymap-editor-contrib/{}", filename),
    }))
}

/// Convert a nickcoutsos key to our format
/// 
/// Conversion rules:
/// - x, y in key units → multiply by 100 for our coordinate system
/// - r (rotation degrees) → multiply by 100 for our rotation units  
/// - rx, ry → multiply by 100
/// - w, h (width/height) → multiply by 100, default to 100
fn convert_key(key: &ContribKey) -> PhysicalKey {
    PhysicalKey {
        // x, y are in key units (1u = 100), convert to our units
        x: (key.x * 100.0) as i32,
        y: (key.y * 100.0) as i32,
        // width/height default to 1u (100)
        width: key.w.map(|w| (w * 100.0) as i32).unwrap_or(100),
        height: key.h.map(|h| (h * 100.0) as i32).unwrap_or(100),
        // rotation in degrees → hundredths of degrees
        rotation: key.r.map(|r| (r * 100.0) as i32).unwrap_or(0),
        // rotation origin
        rx: key.rx.map(|rx| (rx * 100.0) as i32).unwrap_or(0),
        ry: key.ry.map(|ry| (ry * 100.0) as i32).unwrap_or(0),
    }
}

fn generate_rust_code(layouts: &[Layout]) -> String {
    let mut output = String::new();
    output.push_str("// Generated by server/src/bin/fetch_contrib_layouts.rs. DO NOT EDIT.\n");
    output.push_str("// Source: https://github.com/nickcoutsos/keymap-editor-contrib\n");
    output.push_str("// To regenerate: bazel run //server:fetch_contrib_layouts\n\n");
    output.push_str("use super::{PhysicalKey, KeyOrigin};\n\n");
    output.push_str("pub struct ContribLayout {\n");
    output.push_str("    pub name: &'static str,\n");
    output.push_str("    pub display_name: Option<&'static str>,\n");
    output.push_str("    pub keys: &'static [PhysicalKey],\n");
    output.push_str("    pub source_file: &'static str,\n");
    output.push_str("}\n\n");
    output.push_str("pub const CONTRIB_LAYOUTS: &[ContribLayout] = &[\n");

    for layout in layouts {
        output.push_str("    ContribLayout {\n");
        output.push_str(&format!("        name: \"{}\",\n", layout.name));
        if let Some(dn) = &layout.display_name {
            output.push_str(&format!("        display_name: Some(\"{}\"),\n", dn));
        } else {
            output.push_str("        display_name: None,\n");
        }
        output.push_str("        keys: &[\n");
        for key in &layout.keys {
            output.push_str(&format!(
                "            PhysicalKey {{ width: {}, height: {}, x: {}, y: {}, rotation: {}, rx: {}, ry: {}, origin: KeyOrigin::Standard, name: String::new() }},\n",
                key.width, key.height, key.x, key.y, key.rotation, key.rx, key.ry
            ));
        }
        output.push_str("        ],\n");
        output.push_str(&format!("        source_file: \"{}\",\n", layout.source_file));
        output.push_str("    },\n");
    }

    output.push_str("];\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_key_standard() {
        let contrib_key = ContribKey {
            row: 0,
            col: 0,
            r: None,
            x: 0.0,
            y: 0.0,
            rx: None,
            ry: None,
            w: None,
            h: None,
        };

        let key = convert_key(&contrib_key);
        assert_eq!(key.x, 0);
        assert_eq!(key.y, 0);
        assert_eq!(key.width, 100);
        assert_eq!(key.height, 100);
        assert_eq!(key.rotation, 0);
    }

    #[test]
    fn test_convert_key_with_rotation() {
        let contrib_key = ContribKey {
            row: 0,
            col: 0,
            r: Some(-15.0),
            x: 0.11,
            y: 1.72,
            rx: Some(0.61),
            ry: Some(2.22),
            w: Some(1.0),
            h: Some(1.0),
        };

        let key = convert_key(&contrib_key);
        assert_eq!(key.x, 11);  // 0.11 * 100
        assert_eq!(key.y, 172); // 1.72 * 100
        assert_eq!(key.rotation, -1500); // -15.0 * 100
        assert_eq!(key.rx, 61);  // 0.61 * 100
        assert_eq!(key.ry, 222); // 2.22 * 100
    }

    #[test]
    fn test_convert_key_wide() {
        let contrib_key = ContribKey {
            row: 0,
            col: 0,
            r: None,
            x: 2.0,
            y: 0.0,
            rx: None,
            ry: None,
            w: Some(2.25),  // 2.25u key (like left shift)
            h: Some(1.0),
        };

        let key = convert_key(&contrib_key);
        assert_eq!(key.x, 200);
        assert_eq!(key.width, 225); // 2.25 * 100
    }
}
