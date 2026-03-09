use anyhow::{Context, Result};
use std::fs;
use thockflow::keymap::layouts::ZMK_LAYOUTS;
use thockflow::keymap::{generate_svg, parse_raw_bindings, KeymapData, Layer, PhysicalKey};

fn parse_keymap(content: &str) -> Result<KeymapData> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_devicetree::LANGUAGE.into())?;
    let tree = parser
        .parse(content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse DTS"))?;

    let root_node = tree.root_node();
    if root_node.has_error() {
        fn find_error(node: tree_sitter::Node, _source: &[u8], pos: &mut String) {
            if node.has_error() {
                if node.kind() == "ERROR" {
                    *pos = format!(
                        "Parse error at line {}, column {}",
                        node.start_position().row + 1,
                        node.start_position().column + 1
                    );
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        find_error(child, _source, pos);
                        if !pos.is_empty() {
                            return;
                        }
                    }
                }
            }
        }
        let mut error_pos = String::new();
        find_error(root_node, content.as_bytes(), &mut error_pos);
        if !error_pos.is_empty() {
            return Err(anyhow::anyhow!(error_pos));
        }
    }

    let mut physical_layout = Vec::new();
    let mut layers = Vec::new();

    let include_re = regex::Regex::new(r#"(?m)^#include\s*[<"](.+?)[>"]"#).unwrap();
    let includes: Vec<String> = include_re
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect();

    fn traverse(
        node: tree_sitter::Node,
        source: &[u8],
        physical_layout: &mut Vec<PhysicalKey>,
        layers: &mut Vec<Layer>,
    ) {
        if node.kind() == "node" {
            let node_name = node
                .child_by_field_name("name")
                .map(|n| n.utf8_text(source).unwrap_or(""))
                .unwrap_or("");

            let mut is_phys = false;
            let mut is_keymap = false;

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "property" {
                    let prop_name = child
                        .child_by_field_name("name")
                        .map(|n| n.utf8_text(source).unwrap_or(""))
                        .unwrap_or("");
                    if prop_name == "compatible" {
                        let prop_value = child
                            .child_by_field_name("value")
                            .map(|n| n.utf8_text(source).unwrap_or(""))
                            .unwrap_or("");
                        if prop_value.contains("zmk,physical-layout") {
                            is_phys = true;
                        } else if prop_value.contains("zmk,keymap") {
                            is_keymap = true;
                        }
                    }
                }
            }

            if is_phys {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "property" {
                        let prop_name = child
                            .child_by_field_name("name")
                            .map(|n| n.utf8_text(source).unwrap_or(""))
                            .unwrap_or("");
                        if prop_name == "keys" {
                            let mut cursor = child.walk();
                            for val_node in child.children(&mut cursor) {
                                if val_node.kind() != "identifier" {
                                    let raw_val = val_node.utf8_text(source).unwrap_or("");
                                    let num_re = r"\(?([\d-]+)\)?";
                                    // Format: width, height, x, y, rotation, rx, ry
                                    let key_re_str = format!(
                                        r"&key_physical_attrs\s+{}\s+{}\s+{}\s+{}\s+{}\s+{}\s+{}",
                                        num_re, num_re, num_re, num_re, num_re, num_re, num_re
                                    );
                                    let key_regex = regex::Regex::new(&key_re_str).unwrap();
                                    for cap in key_regex.captures_iter(raw_val) {
                                        physical_layout.push(PhysicalKey {
                                            width: cap[1].parse().unwrap_or(100),
                                            height: cap[2].parse().unwrap_or(100),
                                            x: cap[3].parse().unwrap_or(0),
                                            y: cap[4].parse().unwrap_or(0),
                                            rotation: cap[5].parse().unwrap_or(0),
                                            rx: cap[6].parse().unwrap_or(0),
                                            ry: cap[7].parse().unwrap_or(0),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if is_keymap {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "node" {
                        let mut layer_name = String::new();
                        let mut bindings = Vec::new();
                        let mut inner_cursor = child.walk();
                        for inner_child in child.children(&mut inner_cursor) {
                            if inner_child.kind() == "node_name"
                                || inner_child.kind() == "identifier"
                            {
                                if layer_name.is_empty() {
                                    layer_name =
                                        inner_child.utf8_text(source).unwrap_or("").to_string();
                                }
                            }
                            if inner_child.kind() == "property" {
                                let prop_name = inner_child
                                    .child_by_field_name("name")
                                    .map(|n| n.utf8_text(source).unwrap_or(""))
                                    .unwrap_or("");
                                if prop_name == "bindings" {
                                    let mut prop_cursor = inner_child.walk();
                                    for val_node in inner_child.children(&mut prop_cursor) {
                                        if val_node.kind() != "identifier" {
                                            let raw_val = val_node.utf8_text(source).unwrap_or("");
                                            bindings.extend(parse_raw_bindings(raw_val));
                                        }
                                    }
                                }
                            }
                        }
                        if !bindings.is_empty() {
                            layers.push(Layer {
                                name: layer_name,
                                bindings,
                            });
                        }
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            traverse(child, source, physical_layout, layers);
        }
    }

    traverse(
        root_node,
        content.as_bytes(),
        &mut physical_layout,
        &mut layers,
    );

    if !layers.is_empty() {
        let first_layer_len = layers[0].bindings.len();
        for (i, layer) in layers.iter().enumerate() {
            if layer.bindings.len() != first_layer_len {
                return Err(anyhow::anyhow!(
                    "Layer '{}' (index {}) has {} bindings, but first layer has {}",
                    layer.name,
                    i,
                    layer.bindings.len(),
                    first_layer_len
                ));
            }
        }
    }

    if physical_layout.is_empty() && !layers.is_empty() {
        let key_count = layers[0].bindings.len();
        let matches: Vec<_> = ZMK_LAYOUTS
            .iter()
            .filter(|l| l.keys.len() == key_count)
            .collect();

        if !matches.is_empty() {
            let matched_layout = matches
                .iter()
                .find(|l| {
                    l.name.contains("default")
                        || l.display_name
                            .map_or(false, |dn| dn.to_lowercase().contains("default"))
                })
                .or_else(|| matches.iter().find(|l| l.name.contains("6col")))
                .unwrap_or(&matches[0]);

            eprintln!(
                "Matched layout: {} from {}",
                matched_layout.name, matched_layout.source_file
            );
            physical_layout = matched_layout
                .keys
                .iter()
                .map(|k| PhysicalKey {
                    width: k.width,
                    height: k.height,
                    x: k.x,
                    y: k.y,
                    rotation: k.rotation,
                    rx: k.rx,
                    ry: k.ry,
                })
                .collect();
        }
    }

    if physical_layout.is_empty() {
        return Err(anyhow::anyhow!(
            "Missing physical layout and no match found for {} keys",
            layers.get(0).map_or(0, |l| l.bindings.len())
        ));
    }
    if layers.is_empty() {
        return Err(anyhow::anyhow!("Missing keymap layers"));
    }

    Ok(KeymapData {
        physical_layout,
        layers,
        includes,
        aliases: std::collections::HashMap::new(),
        defsrc: Vec::new(),
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <keymap_file> [output.svg]", args[0]);
        std::process::exit(1);
    }

    let content =
        fs::read_to_string(&args[1]).with_context(|| format!("Failed to read {}", args[1]))?;
    let data = parse_keymap(&content)?;
    let svg = generate_svg(&data);

    if let Some(output_path) = args.get(2) {
        fs::write(output_path, &svg).with_context(|| format!("Failed to write {}", output_path))?;
        eprintln!("Wrote SVG to {}", output_path);
    } else {
        println!("{}", svg);
    }

    Ok(())
}
