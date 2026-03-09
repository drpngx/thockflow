use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser};

#[cfg(feature = "runfiles")]
use runfiles::Runfiles;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParameterType {
    Layer,
    Keycode,
    Modifier,
    Constant,
    None,
}

#[derive(Debug)]
struct Behavior {
    name: String,
    label: Option<String>,
    display_name: Option<String>,
    binding_cells: u32,
    include_file: String,
    is_default: bool,
    compatible: Option<String>,
    parameter_metadata: Vec<ParameterType>,
    c_include: Option<String>,
    constants: Vec<String>,
}

fn parse_includes_recursively(
    base_dir: &Path,
    current_file: &Path,
    included_files: &mut HashSet<String>,
) -> Result<()> {
    if let Ok(content) = fs::read_to_string(current_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("#include") {
                let is_angle_bracket = line.contains('<');
                if let Some(start) = line.find('<').or_else(|| line.find('"')) {
                    if let Some(end) = line.rfind('>').or_else(|| line.rfind('"')) {
                        if start < end {
                            let include_path_str = &line[start + 1..end];
                            if let Some(filename) = Path::new(include_path_str)
                                .file_name()
                                .and_then(|n| n.to_str())
                            {
                                if is_angle_bracket {
                                    if included_files.insert(filename.to_string()) {
                                        let search_paths = vec![
                                            base_dir.join("dts").join(include_path_str),
                                            base_dir.join("dts").join("behaviors").join(filename),
                                            base_dir.join("dts").join(filename),
                                        ];
                                        for p in search_paths {
                                            if p.exists() {
                                                let _ = parse_includes_recursively(
                                                    base_dir,
                                                    &p,
                                                    included_files,
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn get_c_include_and_header(comp: &str) -> Option<(&'static str, &'static str)> {
    match comp {
        "zmk,behavior-backlight" => Some((
            "dt-bindings/zmk/backlight.h",
            "include/dt-bindings/zmk/backlight.h",
        )),
        "zmk,behavior-rgb-underglow" => {
            Some(("dt-bindings/zmk/rgb.h", "include/dt-bindings/zmk/rgb.h"))
        }
        "zmk,behavior-outputs" => Some((
            "dt-bindings/zmk/outputs.h",
            "include/dt-bindings/zmk/outputs.h",
        )),
        "zmk,behavior-bluetooth" => Some(("dt-bindings/zmk/bt.h", "include/dt-bindings/zmk/bt.h")),
        "zmk,behavior-mouse-key-press" | "zmk,behavior-input-two-axis" => Some((
            "dt-bindings/zmk/pointing.h",
            "include/dt-bindings/zmk/pointing.h",
        )),
        "zmk,behavior-ext-power" => Some((
            "dt-bindings/zmk/ext_power.h",
            "include/dt-bindings/zmk/ext_power.h",
        )),
        _ => None,
    }
}

fn parse_constants_from_header(zmk_path: &Path, header_rel_path: &str) -> Vec<String> {
    let mut constants = Vec::new();
    let header_path = zmk_path.join("app").join(header_rel_path);
    if let Ok(content) = fs::read_to_string(header_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("#define") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let macro_name = parts[1];
                    if macro_name.contains('(')
                        || macro_name.ends_with("_CMD")
                        || macro_name.starts_with("ZMK_")
                    {
                        continue;
                    }
                    if macro_name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        constants.push(macro_name.to_string());
                    }
                }
            }
        }
    }
    constants
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let (zmk_path_buf, output_path) = if args.len() >= 2 {
        let zmk = std::path::PathBuf::from(&args[1]);
        let out = if args.len() >= 3 {
            Some(std::path::PathBuf::from(&args[2]))
        } else {
            None
        };
        (zmk, out)
    } else {
        #[cfg(feature = "runfiles")]
        {
            let r = Runfiles::create()?;
            let dtsi_path = runfiles::rlocation!(r, "zmk/app/dts/behaviors.dtsi");

            if let Some(dtsi_path) = dtsi_path {
                if dtsi_path.exists() {
                    let zmk = dtsi_path
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .map(|p| p.to_path_buf())
                        .context("Could not find zmk root from behaviors.dtsi path")?;

                    let out = if let Some(ws_root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
                        let mut p = std::path::PathBuf::from(ws_root);
                        p.push("src/keymap/behaviors.rs");
                        Some(p)
                    } else {
                        Some(std::path::PathBuf::from("src/keymap/behaviors.rs"))
                    };
                    (zmk, out)
                } else {
                    eprintln!("Usage: {} <zmk_path> [output_path]", args[0]);
                    eprintln!(
                        "Could not find zmk behaviors.dtsi in runfiles at {:?}",
                        dtsi_path
                    );
                    std::process::exit(1);
                }
            } else {
                eprintln!("Usage: {} <zmk_path> [output_path]", args[0]);
                eprintln!("rlocation! macro returned None for zmk behaviors");
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "runfiles"))]
        {
            eprintln!("Usage: {} <zmk_path> [output_path]", args[0]);
            eprintln!("Runfiles support not compiled in. Please provide paths manually.");
            std::process::exit(1);
        }
    };

    let zmk_path = zmk_path_buf.as_path();
    let app_dir = zmk_path.join("app");
    let behaviors_dtsi_path = app_dir.join("dts/behaviors.dtsi");
    let behaviors_dir = app_dir.join("dts/behaviors");

    let mut behaviors = Vec::new();

    let mut included_files = HashSet::new();
    if let Some(filename) = behaviors_dtsi_path.file_name().and_then(|n| n.to_str()) {
        included_files.insert(filename.to_string());
    }
    let _ = parse_includes_recursively(&app_dir, &behaviors_dtsi_path, &mut included_files);

    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_devicetree::LANGUAGE.into())?;

    if behaviors_dir.exists() {
        for entry in fs::read_dir(&behaviors_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .extension()
                .map_or(false, |ext| ext == "dtsi" || ext == "h")
            {
                let filename = path.file_name().unwrap().to_str().unwrap().to_string();
                let is_default = included_files.contains(&filename);

                let content = fs::read_to_string(&path)?;
                let tree = parser
                    .parse(&content, None)
                    .context("Failed to parse DTS")?;

                let include_path = format!("behaviors/{}", filename);
                extract_behaviors(
                    tree.root_node(),
                    &content,
                    &include_path,
                    is_default,
                    &mut behaviors,
                    zmk_path,
                )?;
            }
        }
    }

    let mut output = String::new();
    output.push_str("// Generated by server/src/bin/zmk_behaviors.rs. DO NOT EDIT.\n");
    output.push_str("// To regenerate, run:\n");
    output.push_str("// bazel run //server:zmk_behaviors\n\n");
    output.push_str("#[derive(Debug, Clone, Copy, PartialEq)]\n");
    output.push_str("pub enum ParameterType {\n");
    output.push_str("    Layer,\n");
    output.push_str("    Keycode,\n");
    output.push_str("    Modifier,\n");
    output.push_str("    Constant,\n");
    output.push_str("    None,\n");
    output.push_str("}\n\n");
    output.push_str("#[allow(dead_code)]\n");
    output.push_str("#[derive(Debug, Clone)]\n");
    output.push_str("pub struct ZmkBehavior {\n");
    output.push_str("    pub name: &'static str,\n");
    output.push_str("    pub label: Option<&'static str>,\n");
    output.push_str("    pub display_name: Option<&'static str>,\n");
    output.push_str("    pub binding_cells: u32,\n");
    output.push_str("    pub include_file: &'static str,\n");
    output.push_str("    pub is_default: bool,\n");
    output.push_str("    pub compatible: Option<&'static str>,\n");
    output.push_str("    pub parameter_metadata: &'static [ParameterType],\n");
    output.push_str("    pub c_include: Option<&'static str>,\n");
    output.push_str("    pub constants: &'static [&'static str],\n");
    output.push_str("}\n\n");
    output.push_str("pub const ZMK_BEHAVIORS: &[ZmkBehavior] = &[\n");

    for b in &behaviors {
        output.push_str("    ZmkBehavior {\n");
        output.push_str(&format!("        name: \"{}\",\n", b.name));
        if let Some(label) = &b.label {
            output.push_str(&format!("        label: Some(\"{}\"),\n", label));
        } else {
            output.push_str("        label: None,\n");
        }
        if let Some(dn) = &b.display_name {
            output.push_str(&format!("        display_name: Some(\"{}\"),\n", dn));
        } else {
            output.push_str("        display_name: None,\n");
        }
        output.push_str(&format!("        binding_cells: {},\n", b.binding_cells));
        output.push_str(&format!("        include_file: \"{}\",\n", b.include_file));
        output.push_str(&format!("        is_default: {},\n", b.is_default));
        if let Some(comp) = &b.compatible {
            output.push_str(&format!("        compatible: Some(\"{}\"),\n", comp));
        } else {
            output.push_str("        compatible: None,\n");
        }

        output.push_str("        parameter_metadata: &[\n");
        for pt in &b.parameter_metadata {
            output.push_str(&format!("            ParameterType::{:?},\n", pt));
        }
        output.push_str("        ],\n");

        if let Some(c_inc) = &b.c_include {
            output.push_str(&format!("        c_include: Some(\"{}\"),\n", c_inc));
        } else {
            output.push_str("        c_include: None,\n");
        }

        output.push_str("        constants: &[\n");
        for c in &b.constants {
            output.push_str(&format!("            \"{}\",\n", c));
        }
        output.push_str("        ],\n");

        output.push_str("    },\n");
    }
    output.push_str("];\n");

    if let Some(out_path) = output_path {
        fs::write(out_path, output)?;
    } else {
        println!("{}", output);
    }

    Ok(())
}

fn extract_behaviors(
    node: Node,
    source: &str,
    filename: &str,
    is_default: bool,
    behaviors: &mut Vec<Behavior>,
    zmk_path: &Path,
) -> Result<()> {
    if node.kind() == "node" {
        let mut node_name = String::new();
        let mut handle = None;
        let mut identifiers = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                identifiers.push(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
            } else if child.kind() == "{" {
                break;
            }
        }

        if identifiers.len() >= 2 {
            handle = Some(identifiers[0].clone());
            node_name = identifiers[1].clone();
        } else if identifiers.len() == 1 {
            node_name = identifiers[0].clone();
        }

        if node_name.is_empty() {
            node_name = node
                .child_by_field_name("name")
                .map(|n| n.utf8_text(source.as_bytes()).unwrap_or(""))
                .unwrap_or("")
                .to_string();
        }

        let mut display_name = None;
        let mut binding_cells = None;
        let mut compatible = None;
        let mut bindings_behavior_labels = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "property" {
                let prop_name = child
                    .child_by_field_name("name")
                    .map(|n| n.utf8_text(source.as_bytes()).unwrap_or(""))
                    .unwrap_or("");

                if prop_name == "display-name" || prop_name == "label" {
                    let val = child
                        .child_by_field_name("value")
                        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or(""))
                        .unwrap_or("");
                    display_name = Some(val.trim_matches('\"').to_string());
                } else if prop_name == "compatible" {
                    let val = child
                        .child_by_field_name("value")
                        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or(""))
                        .unwrap_or("");
                    compatible = Some(val.trim_matches('\"').to_string());
                } else if prop_name == "#binding-cells" {
                    let val = child
                        .child_by_field_name("value")
                        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or(""))
                        .unwrap_or("");
                    if let Some(num_str) = val
                        .trim_matches(|c| c == '<' || c == '>' || c == ' ')
                        .split_whitespace()
                        .next()
                    {
                        if let Ok(num) = num_str.parse::<u32>() {
                            binding_cells = Some(num);
                        }
                    }
                } else if prop_name == "bindings" {
                    if let Some(value_node) = child.child_by_field_name("value") {
                        fn find_refs(node: Node, source: &[u8], refs: &mut Vec<String>) {
                            let text = node.utf8_text(source).unwrap_or("");
                            if text.starts_with('&') {
                                refs.push(text[1..].to_string());
                            }
                            let mut cursor = node.walk();
                            for child in node.children(&mut cursor) {
                                find_refs(child, source, refs);
                            }
                        }
                        find_refs(value_node, source.as_bytes(), &mut bindings_behavior_labels);
                    }
                }
            }
        }

        if let Some(cells) = binding_cells {
            let mut parameter_metadata = Vec::new();
            if let Some(comp) = &compatible {
                match comp.as_str() {
                    "zmk,behavior-momentary-layer"
                    | "zmk,behavior-to-layer"
                    | "zmk,behavior-toggle-layer"
                    | "zmk,behavior-sticky-layer" => {
                        parameter_metadata.push(ParameterType::Layer);
                    }
                    "zmk,behavior-layer-tap" => {
                        parameter_metadata.push(ParameterType::Layer);
                        parameter_metadata.push(ParameterType::Keycode);
                    }
                    "zmk,behavior-key-press" => {
                        parameter_metadata.push(ParameterType::Keycode);
                    }
                    "zmk,behavior-mod-tap" => {
                        parameter_metadata.push(ParameterType::Modifier);
                        parameter_metadata.push(ParameterType::Keycode);
                    }
                    "zmk,behavior-sticky-key" => {
                        parameter_metadata.push(ParameterType::Modifier);
                    }
                    "zmk,behavior-hold-tap" => {
                        for (i, label) in bindings_behavior_labels.iter().enumerate() {
                            if i as u32 >= cells {
                                break;
                            }
                            match label.as_str() {
                                "mo" | "to" | "tog" => {
                                    parameter_metadata.push(ParameterType::Layer)
                                }
                                "sk" => parameter_metadata.push(ParameterType::Modifier),
                                "kp" => {
                                    if i == 0
                                        && (node_name == "mod_tap"
                                            || handle.as_deref() == Some("mt"))
                                    {
                                        parameter_metadata.push(ParameterType::Modifier);
                                    } else {
                                        parameter_metadata.push(ParameterType::Keycode);
                                    }
                                }
                                _ => parameter_metadata.push(ParameterType::Keycode),
                            }
                        }
                    }
                    "zmk,behavior-bluetooth" => {
                        for _ in 0..cells {
                            parameter_metadata.push(ParameterType::Constant);
                        }
                    }
                    "zmk,behavior-outputs"
                    | "zmk,behavior-backlight"
                    | "zmk,behavior-rgb-underglow"
                    | "zmk,behavior-ext-power"
                    | "zmk,behavior-input-two-axis"
                    | "zmk,behavior-mouse-key-press" => {
                        for _ in 0..cells {
                            parameter_metadata.push(ParameterType::Constant);
                        }
                    }
                    _ => {
                        for _ in 0..cells {
                            parameter_metadata.push(ParameterType::None);
                        }
                    }
                }
            } else {
                for _ in 0..cells {
                    parameter_metadata.push(ParameterType::None);
                }
            }

            while parameter_metadata.len() < cells as usize {
                parameter_metadata.push(ParameterType::None);
            }

            let mut c_include = None;
            let mut constants = Vec::new();
            if let Some(comp) = &compatible {
                if let Some((c_inc, header_path)) = get_c_include_and_header(comp) {
                    c_include = Some(c_inc.to_string());
                    constants = parse_constants_from_header(zmk_path, header_path);
                }
            }

            behaviors.push(Behavior {
                name: node_name,
                label: handle,
                display_name,
                binding_cells: cells,
                include_file: filename.to_string(),
                is_default,
                compatible: compatible.clone(),
                parameter_metadata,
                c_include,
                constants,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_behaviors(child, source, filename, is_default, behaviors, zmk_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_includes_not_recursive_for_quoted_local_includes() -> Result<()> {
        let dir = tempdir()?;
        let app_dir = dir.path().join("app");
        let dts_dir = app_dir.join("dts");
        let behaviors_dir = dts_dir.join("behaviors");
        fs::create_dir_all(&behaviors_dir)?;

        let behaviors_dtsi = dts_dir.join("behaviors.dtsi");
        fs::write(&behaviors_dtsi, "#include <behaviors/mouse_keys.dtsi>")?;

        let mouse_keys_dtsi = behaviors_dir.join("mouse_keys.dtsi");
        fs::write(&mouse_keys_dtsi, "#include \"mouse_move.dtsi\"")?;

        let mouse_move_dtsi = behaviors_dir.join("mouse_move.dtsi");
        fs::write(&mouse_move_dtsi, "/ { };")?;

        let mut included_files = HashSet::new();
        included_files.insert("behaviors.dtsi".to_string());
        parse_includes_recursively(&app_dir, &behaviors_dtsi, &mut included_files)?;

        // If it's working correctly according to the user's requirement,
        // it SHOULD NOT have mouse_move.dtsi if we only want direct/transitive
        // includes from behaviors.dtsi, but wait, the user says
        // "Understand that mouse_move.dtsi is not mouse_keys.dtsi. OK?"
        // and "test that checks that the mouse_move is not is_default, because we know it isn't"

        // My current parser DOES find it.
        assert!(
            !included_files.contains("mouse_move.dtsi"),
            "mouse_move.dtsi should not be in included_files"
        );

        Ok(())
    }

    #[test]
    fn test_parse_constants_from_header() -> Result<()> {
        let dir = tempdir()?;
        let app_dir = dir.path().join("app");
        let include_dir = app_dir.join("include/dt-bindings/zmk");
        fs::create_dir_all(&include_dir)?;

        let header_path = include_dir.join("test_header.h");
        fs::write(
            &header_path,
            "
#define BL_ON_CMD 0
#define BL_OFF_CMD 1
#define BL_TOG_CMD 2

#define BL_ON BL_ON_CMD 0
#define BL_OFF BL_OFF_CMD 0
#define BL_TOG BL_TOG_CMD 0
#define ZMK_NOT_CONSTANT 1
#define WITH_ARGS(x) (x)
",
        )?;

        let constants =
            parse_constants_from_header(dir.path(), "include/dt-bindings/zmk/test_header.h");

        assert_eq!(constants.len(), 3);
        assert!(constants.contains(&"BL_ON".to_string()));
        assert!(constants.contains(&"BL_OFF".to_string()));
        assert!(constants.contains(&"BL_TOG".to_string()));

        Ok(())
    }

    #[test]
    fn test_get_c_include_and_header() {
        assert_eq!(
            get_c_include_and_header("zmk,behavior-backlight"),
            Some((
                "dt-bindings/zmk/backlight.h",
                "include/dt-bindings/zmk/backlight.h"
            ))
        );
        assert_eq!(
            get_c_include_and_header("zmk,behavior-input-two-axis"),
            Some((
                "dt-bindings/zmk/pointing.h",
                "include/dt-bindings/zmk/pointing.h"
            ))
        );
        assert_eq!(
            get_c_include_and_header("zmk,behavior-mouse-key-press"),
            Some((
                "dt-bindings/zmk/pointing.h",
                "include/dt-bindings/zmk/pointing.h"
            ))
        );
        assert_eq!(get_c_include_and_header("zmk,behavior-unknown"), None);
    }
}
