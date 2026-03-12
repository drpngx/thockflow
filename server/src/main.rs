use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::marker::PhantomData;

use anyhow::Result;
use axum::body::{Body, BoxBody};
use axum::extract::Query;
use axum::http::{header, HeaderMap, HeaderValue, Request, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{get_service, post, MethodRouter};
use axum::Extension;
use axum::{routing::get, Json, Router};
use futures::future::BoxFuture;
use futures::ready;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thockflow::keymap::{parse_raw_bindings, Defvar, KeymapData, KeyOrigin, Layer, LayerType, PhantomKey, PhysicalKey, ProcessUnmappedKeys, VarType};
use thockflow::ServerAppProps;
use tree_sitter_devicetree;
use tree_sitter_scheme;
use tokio_util::task::LocalPoolHandle;
use tower::Service;
use tower_http::services::ServeDir;
use yew_router::Routable;

use thockflow::keymap::behaviors::ZMK_BEHAVIORS;
use thockflow::keymap::layouts::ZMK_LAYOUTS;

lazy_static::lazy_static!(
    // Use the source HTML as a template
    static ref INDEX_HTML: String = {
        String::from_utf8(std::fs::read("bundle/dist/index.html").unwrap().try_into().unwrap()).unwrap()
    };
    static ref APP_WASM_PATH: &'static str = {
        option_env!("APP_WASM_PATH").unwrap_or("/app_wasm_bg.wasm")
    };
    static ref APP_JS_PATH: &'static str = {
        option_env!("APP_JS_PATH").unwrap_or("/app_wasm.js")
    };

);

use log::{error, info};

#[derive(Deserialize, Serialize)]
struct KeymapRequest {
    content: String,
    #[serde(default)]
    is_mac: bool,
    #[serde(default)]
    is_laptop: bool,
}

#[derive(Deserialize, Serialize)]
struct SaveKeymapRequest {
    original_content: String,
    data: KeymapData,
}

#[derive(Serialize)]
struct SaveKeymapResponse {
    content: String,
}

#[derive(Deserialize, Serialize)]
struct PatchKeymapRequest {
    file_content: String,
    data: KeymapData,
}

#[derive(Serialize)]
struct PatchKeymapResponse {
    content: String,
}

async fn parse_keymap_api(Json(req): Json<KeymapRequest>) -> impl IntoResponse {
    info!(
        "Received parse request, content length: {}",
        req.content.len()
    );
    match parse_keymap_with_tree_sitter(&req.content) {
        Ok(data) => {
            info!(
                "Successfully parsed keymap with {} keys and {} layers",
                data.physical_layout.len(),
                data.layers.len()
            );
            (StatusCode::OK, Json(data)).into_response()
        }
        Err(e) => {
            error!("Parse error: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

async fn save_keymap_api(Json(req): Json<SaveKeymapRequest>) -> impl IntoResponse {
    info!("Received save request");
    match generate_keymap_dts(&req.original_content, &req.data) {
        Ok(content) => {
            info!(
                "Successfully generated new keymap DTS, length: {}",
                content.len()
            );
            (StatusCode::OK, Json(SaveKeymapResponse { content })).into_response()
        }
        Err(e) => {
            error!("Generation error: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

async fn patch_keymap_api(Json(req): Json<PatchKeymapRequest>) -> impl IntoResponse {
    info!("Received patch request, file content length: {}", req.file_content.len());
    
    // First, validate the uploaded file by parsing it
    if let Err(e) = parse_keymap_with_tree_sitter(&req.file_content) {
        error!("Invalid keymap file uploaded: {}", e);
        return (StatusCode::BAD_REQUEST, format!("Invalid keymap file: {}", e)).into_response();
    }
    
    // Generate the patched keymap by merging the layers from data into the uploaded file
    match generate_keymap_dts(&req.file_content, &req.data) {
        Ok(content) => {
            // Validate the generated content by parsing it again
            match parse_keymap_with_tree_sitter(&content) {
                Ok(_) => {
                    info!(
                        "Successfully patched and validated keymap, new length: {}",
                        content.len()
                    );
                    (StatusCode::OK, Json(PatchKeymapResponse { content })).into_response()
                }
                Err(e) => {
                    error!("Patched keymap failed validation: {}", e);
                    // Find the problematic line
                    let error_str = e.to_string();
                    if let Some(line_str) = error_str.split("line ").nth(1) {
                        if let Some(line_num) = line_str.split(",").next().and_then(|s| s.trim().parse::<usize>().ok()) {
                            error!("Error around line {}:", line_num);
                            for (i, line) in content.lines().enumerate() {
                                let line_no = i + 1;
                                if line_no >= line_num.saturating_sub(2) && line_no <= line_num + 2 {
                                    error!("  {}: {}", line_no, line);
                                }
                            }
                        }
                    }
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Patched file is invalid: {}", e)).into_response()
                }
            }
        }
        Err(e) => {
            error!("Patch generation error: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

fn generate_keymap_dts(original: &str, data: &KeymapData) -> Result<String> {
    let mut content = if original.is_empty() {
        // Create a minimal ZMK keymap template
        let mut template = String::from("/ {\n    keymap {\n        compatible = \"zmk,keymap\";\n");
        for (i, layer) in data.layers.iter().enumerate() {
            let layer_name = if layer.name.is_empty() { format!("layer_{}", i) } else { layer.name.clone() };
            // Sanitize layer name for DTS node
            let node_name = layer_name.to_lowercase().replace(' ', "_").replace(|c: char| !c.is_alphanumeric() && c != '_', "");
            template.push_str(&format!("        {} {{\n            bindings = <>;\n        }};\n", node_name));
        }
        template.push_str("    };\n};\n");
        template
    } else {
        original.to_string()
    };

    // 1. Handle #includes
    let include_re = regex::Regex::new(r#"(?m)^#include\s*[<"](.+?)[>"]"#).unwrap();
    let mut existing_includes: std::collections::HashSet<String> = include_re
        .captures_iter(original)
        .map(|cap| cap[1].to_string())
        .collect();

    let mut new_includes = Vec::new();

    // a) Add includes from data.includes
    for inc in &data.includes {
        if !existing_includes.contains(inc) {
            new_includes.push(format!("#include <{}>", inc));
            existing_includes.insert(inc.clone());
        }
    }

    // b) Add includes from non-default behaviors used in layers
    for layer in &data.layers {
        for binding in &layer.bindings {
            let tokens: Vec<&str> = binding.split_whitespace().collect();
            if let Some(token) = tokens.first() {
                let token = token
                    .trim_matches(|c| c == '&' || c == '<' || c == '>' || c == ';' || c == ' ');
                if let Some(behavior) = ZMK_BEHAVIORS
                    .iter()
                    .find(|b| b.label == Some(token) || b.name == token)
                {
                    if !behavior.is_default && !existing_includes.contains(behavior.include_file) {
                        new_includes.push(format!("#include <{}>", behavior.include_file));
                        existing_includes.insert(behavior.include_file.to_string());
                    }
                    if let Some(c_inc) = behavior.c_include {
                        if !existing_includes.contains(c_inc) {
                            new_includes.push(format!("#include <{}>", c_inc));
                            existing_includes.insert(c_inc.to_string());
                        }
                    }
                }
            }
        }
    }

    if !new_includes.is_empty() {
        let last_include_pos = include_re.find_iter(original).last().map(|m| m.end());

        if let Some(pos) = last_include_pos {
            let mut insert_text = String::from("\n");
            insert_text.push_str(&new_includes.join("\n"));
            content.insert_str(pos, &insert_text);
        } else {
            content.insert_str(0, &(new_includes.join("\n") + "\n\n"));
        }
    }

    // 2. Justification Logic: Compute max width for each key across all layers
    let num_keys = data.physical_layout.len();
    let mut max_widths = vec![0; num_keys];
    for layer in &data.layers {
        for (i, binding) in layer.bindings.iter().enumerate() {
            if i < num_keys {
                max_widths[i] = max_widths[i].max(binding.len());
            }
        }
    }

    // Identify rows for grouping
    let mut rows = Vec::new();
    if !data.physical_layout.is_empty() {
        let mut current_row = Vec::new();
        let mut row_ref_y = data.physical_layout[0].y;
        for (i, pk) in data.physical_layout.iter().enumerate() {
            // New row if Y changes significantly from the row's reference Y (threshold 14000)
            if i > 0 && (pk.y - row_ref_y).abs() > 14000 {
                rows.push(current_row);
                current_row = Vec::new();
                row_ref_y = pk.y;
            }
            current_row.push(i);
        }
        rows.push(current_row);
    }

    // 3. Find keymap node and its layer nodes
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_devicetree::LANGUAGE.into())?;
    let tree = parser
        .parse(content.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse DTS"))?;

    fn find_keymap_node<'a>(
        node: tree_sitter::Node<'a>,
        source: &[u8],
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "node" {
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
                        if prop_value.contains("zmk,keymap") {
                            return Some(node);
                        }
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(res) = find_keymap_node(child, source) {
                return Some(res);
            }
        }
        None
    }

    let keymap_node = find_keymap_node(tree.root_node(), content.as_bytes())
        .ok_or_else(|| anyhow::anyhow!("Could not find keymap node"))?;

    let mut cursor = keymap_node.walk();
    let original_layer_nodes: Vec<tree_sitter::Node> = keymap_node
        .children(&mut cursor)
        .filter(|n| n.kind() == "node")
        .collect();

    struct Replacement {
        start: usize,
        end: usize,
        text: String,
    }
    let mut replacements = Vec::new();

    // Helper to generate bindings string
    // Note: The value node in tree-sitter is "integer_cells" and includes the < > delimiters
    let gen_bindings = |target_layer: &Layer| {
        let mut new_bindings = String::from("<");
        for row in rows.iter() {
            new_bindings.push_str("\n");
            new_bindings.push_str("                ");
            for (key_in_row, &key_idx) in row.iter().enumerate() {
                if key_in_row > 0 {
                    // Check for large X gap in the physical layout to add extra spacing
                    let pk_curr = &data.physical_layout[key_idx];
                    let pk_prev = &data.physical_layout[row[key_in_row - 1]];
                    let dist = (pk_curr.x - pk_prev.x).abs();
                    let extra_gap = if dist > 40000 {
                        "                                                                         "
                    } else if dist > 15000 {
                        "     "
                    } else {
                        " "
                    };
                    new_bindings.push_str(extra_gap);
                }
                let b = target_layer
                    .bindings
                    .get(key_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("&none");
                if key_in_row == row.len() - 1 {
                    new_bindings.push_str(b);
                } else {
                    let width = max_widths.get(key_idx).cloned().unwrap_or(0);
                    new_bindings.push_str(&format!("{:width$}", b, width = width));
                }
            }
        }
        new_bindings.push_str("\n                        >");
        new_bindings
    };

    // 4. Update existing layers and handle deletions
    for (i, original_node) in original_layer_nodes.iter().enumerate() {
        if i < data.layers.len() {
            let target_layer = &data.layers[i];

            // a) Update Name
            let mut inner_cursor = original_node.walk();
            for child in original_node.children(&mut inner_cursor) {
                if child.kind() == "node_name" || child.kind() == "identifier" {
                    let old_name = child.utf8_text(content.as_bytes()).unwrap_or("");
                    if old_name != target_layer.name {
                        replacements.push(Replacement {
                            start: child.start_byte(),
                            end: child.end_byte(),
                            text: target_layer.name.clone(),
                        });
                    }
                }

                // b) Update Bindings
                if child.kind() == "property" {
                    let prop_name = child
                        .child_by_field_name("name")
                        .map(|n| n.utf8_text(content.as_bytes()).unwrap_or(""))
                        .unwrap_or("");
                    if prop_name == "bindings" {
                        if let Some(value_node) = child.child_by_field_name("value") {
                            replacements.push(Replacement {
                                start: value_node.start_byte(),
                                end: value_node.end_byte(),
                                text: gen_bindings(target_layer),
                            });
                        }
                    }
                }
            }
        } else {
            // Delete extra layer node
            let start = original_node.start_byte();
            let mut end = original_node.end_byte();
            // Include trailing semicolon if present
            if content.as_bytes().get(end) == Some(&b';') {
                end += 1;
            }
            replacements.push(Replacement {
                start,
                end,
                text: String::new(),
            });
        }
    }

    // 5. Add new layers (if data.layers has more than original)
    if data.layers.len() > original_layer_nodes.len() {
        let insert_pos = if let Some(last_node) = original_layer_nodes.last() {
            let mut pos = last_node.end_byte();
            if content.as_bytes().get(pos) == Some(&b';') {
                pos += 1;
            }
            pos
        } else {
            // Fallback: find the closing brace of keymap node
            keymap_node.end_byte() - 2
        };

        let mut new_layers_text = String::new();
        for i in original_layer_nodes.len()..data.layers.len() {
            let target_layer = &data.layers[i];
            new_layers_text.push_str("\n\n                ");
            new_layers_text.push_str(&target_layer.name);
            new_layers_text.push_str(" {\n                        bindings = ");
            new_layers_text.push_str(&gen_bindings(target_layer));
            new_layers_text.push_str(";\n                };");
        }
        replacements.push(Replacement {
            start: insert_pos,
            end: insert_pos,
            text: new_layers_text,
        });
    }

    // Apply replacements in reverse order
    replacements.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in replacements {
        content.replace_range(r.start..r.end, &r.text);
    }

    Ok(content)
}

fn parse_keymap_with_tree_sitter(content: &str) -> Result<KeymapData> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_devicetree::LANGUAGE.into())?;
    let tree = parser
        .parse(content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse DTS"))?;

    let root_node = tree.root_node();
    // ... existing error checking ...
    if root_node.has_error() {
        // Find where the error is
        let mut error_pos = String::new();
        fn find_error(node: tree_sitter::Node, source: &[u8], pos: &mut String) {
            if node.has_error() {
                if node.kind() == "ERROR" {
                    *pos = format!(
                        "Tree-sitter parse error at line {}, column {}",
                        node.start_position().row + 1,
                        node.start_position().column + 1
                    );
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        find_error(child, source, pos);
                        if !pos.is_empty() {
                            return;
                        }
                    }
                }
            }
        }
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

    // Recursive traversal to find nodes
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
            info!("Visiting node: {}", node_name);

            // Check properties for "compatible"
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
                        info!("  Found compatible property: {}", prop_value);
                        if prop_value.contains("zmk,physical-layout") {
                            is_phys = true;
                            info!("  Marked as physical layout");
                        } else if prop_value.contains("zmk,keymap") {
                            is_keymap = true;
                            info!("  Marked as keymap");
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
                            info!("  Parsing keys property...");
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
                                            origin: KeyOrigin::Standard,
                                            name: String::new(),
                                        });
                                    }
                                }
                            }
                            info!("  Found {} keys", physical_layout.len());
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
                            info!(
                                "  Found layer: {} with {} bindings",
                                layer_name,
                                bindings.len()
                            );
                            layers.push(Layer {
                                name: layer_name,
                                bindings,
                                layer_type: LayerType::Deflayer,
                                source_layer: None,
                                key_bindings: std::collections::HashMap::new(),
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
                    "Layer '{}' (index {}) has {} bindings, but first layer has {} bindings. All layers must have the same number of keys.",
                    layer.name, i, layer.bindings.len(), first_layer_len
                ));
            }
        }
    }

    if physical_layout.is_empty() && !layers.is_empty() {
        let key_count = layers[0].bindings.len();
        info!(
            "Physical layout missing, attempting to match by key count: {}",
            key_count
        );

        // Find layouts with matching key count
        let matches: Vec<_> = ZMK_LAYOUTS
            .iter()
            .filter(|l| l.keys.len() == key_count)
            .collect();

        if !matches.is_empty() {
            // Heuristic: prioritize layouts with "default" or "6col"
            let matched_layout = matches
                .iter()
                .find(|l| {
                    l.name.contains("default")
                        || l.display_name
                            .map_or(false, |dn| dn.to_lowercase().contains("default"))
                })
                .or_else(|| matches.iter().find(|l| l.name.contains("6col")))
                .unwrap_or(&matches[0]);

            info!(
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
                    origin: KeyOrigin::Standard,
                    name: String::new(),
                })
                .collect();
        }
    }

    if physical_layout.is_empty() {
        return Err(anyhow::anyhow!("Missing physical layout (zmk,physical-layout compatible node) and no match found in database for {} keys", layers.get(0).map_or(0, |l| l.bindings.len())));
    }
    if layers.is_empty() {
        return Err(anyhow::anyhow!(
            "Missing keymap layers (zmk,keymap compatible node)"
        ));
    }

    Ok(KeymapData {
        physical_layout,
        layers,
        includes,
        aliases: HashMap::new(),
        defsrc: Vec::new(),
        unmapped_names: Vec::new(),
        process_unmapped_keys: ProcessUnmappedKeys::No,
        defvars: Vec::new(),
        phantom_keys: Vec::new(),
        chordsv2: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[test]
    fn test_parse_hshs52_file() {
        let content = include_str!("../../static/hshs52.keymap");
        let result = parse_keymap_with_tree_sitter(content).expect("Should parse successfully");

        // Hillside 52 should have 52 keys
        assert!(
            result.physical_layout.len() >= 52,
            "Expected at least 52 layout keys, got {}",
            result.physical_layout.len()
        );
        assert!(result.layers.len() > 0, "Expected at least one layer");

        // Check some bindings to see if they were grouped correctly
        let first_layer = &result.layers[0];
        assert!(first_layer.bindings.contains(&"&kp GRAVE".to_string()));

        // Check 2-argument binding (bt)
        let adj_layer = &result.layers[5]; // Assuming adj_layer is at index 5
        assert!(adj_layer.name == "adj_layer");
        assert!(adj_layer.bindings.contains(&"&bt BT_SEL 0".to_string()));

        println!(
            "Successfully parsed {} keys and {} layers",
            result.physical_layout.len(),
            result.layers.len()
        );
    }

    #[tokio::test]
    async fn test_parse_keymap_endpoint() {
        // Create a dummy index.html for the test if it doesn't exist to avoid panic in INDEX_HTML
        let _ = std::fs::create_dir_all("bundle/dist");
        if !std::path::Path::new("bundle/dist/index.html").exists() {
            let _ = std::fs::write("bundle/dist/index.html", "<html><body></body></html>");
        }

        let app = app();
        let content = include_str!("../../static/hshs52.keymap");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/parse-keymap")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&KeymapRequest {
                            content: content.to_string(),
                            is_mac: false,
                            is_laptop: false,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let data: KeymapData = serde_json::from_slice(&body).expect("Should return valid JSON");

        assert!(data.physical_layout.len() >= 52);
        assert!(!data.layers.is_empty());
    }

    #[test]
    fn test_zmk_behaviors_metadata() {
        use thockflow::keymap::behaviors::{ParameterType, ZMK_BEHAVIORS};

        let lt = ZMK_BEHAVIORS
            .iter()
            .find(|b| b.label == Some("lt"))
            .expect("lt behavior missing");
        assert_eq!(lt.binding_cells, 2);
        assert_eq!(lt.parameter_metadata.len(), 2);
        assert_eq!(lt.parameter_metadata[0], ParameterType::Layer);
        assert_eq!(lt.parameter_metadata[1], ParameterType::Keycode);

        let mt = ZMK_BEHAVIORS
            .iter()
            .find(|b| b.label == Some("mt"))
            .expect("mt behavior missing");
        assert_eq!(mt.binding_cells, 2);
        assert_eq!(mt.parameter_metadata.len(), 2);
        assert_eq!(mt.parameter_metadata[0], ParameterType::Modifier);
        assert_eq!(mt.parameter_metadata[1], ParameterType::Keycode);

        let bt = ZMK_BEHAVIORS
            .iter()
            .find(|b| b.label == Some("bt"))
            .expect("bt behavior missing");
        assert_eq!(bt.binding_cells, 2);
        assert_eq!(bt.parameter_metadata[0], ParameterType::Constant);
    }

    #[test]
    fn test_keycode_logic() {
        use thockflow::keymap::keycodes;

        // Backspace, arrows, etc. should be regular but NOT modifiers
        assert!(keycodes::is_regular_key("BSPC"));
        assert!(!keycodes::is_modifier("BSPC"));

        assert!(keycodes::is_regular_key("LEFT"));
        assert!(!keycodes::is_modifier("LEFT"));

        assert!(keycodes::is_regular_key("C_AC_BACK"));
        assert!(!keycodes::is_modifier("C_AC_BACK"));
        // Modifiers should be modifiers
        assert!(keycodes::is_modifier("LSHFT"));
        assert!(keycodes::is_modifier("RCTRL"));
        assert!(keycodes::is_modifier("MOD_LSFT"));
    }

    #[test]
    fn test_generate_keymap_dts_deletion() {
        let content = r#"
/ {
    keymap {
        compatible = "zmk,keymap";
        layer_0 {
            bindings = <&kp A &kp B>;
        };
        layer_1 {
            bindings = <&kp C &kp D>;
        };
    };
};
"#;
        let data = KeymapData {
            physical_layout: vec![
                PhysicalKey {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
                PhysicalKey {
                    x: 200,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
            ],
            layers: vec![Layer {
                name: "new_layer_0".to_string(),
                bindings: vec!["&kp X".to_string(), "&kp Y".to_string()],
                layer_type: LayerType::Deflayer,
                source_layer: None,
                key_bindings: HashMap::new(),
            }],
            includes: vec![],
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
        
        phantom_keys: vec![],
            chordsv2: vec![],
    };

        let result = generate_keymap_dts(content, &data).unwrap();
        assert!(result.contains("new_layer_0"));
        assert!(result.contains("&kp X &kp Y"));
        assert!(!result.contains("layer_1"));
        assert!(!result.contains("&kp C &kp D"));
    }

    #[test]
    fn test_generate_keymap_dts_justification() {
        let content = r#"
/ {
    keymap {
        compatible = "zmk,keymap";
        layer_0 {
            bindings = <&kp A &kp B>;
        };
    };
};
"#;
        let data = KeymapData {
            physical_layout: vec![
                PhysicalKey {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
                PhysicalKey {
                    x: 200,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
            ],
            layers: vec![
                Layer {
                    name: "layer_0".to_string(),
                    bindings: vec!["&kp LONG_BINDING".to_string(), "&kp B".to_string()],
                    layer_type: LayerType::Deflayer,
                    source_layer: None,
                    key_bindings: HashMap::new(),
                },
                Layer {
                    name: "layer_1".to_string(),
                    bindings: vec!["&kp A".to_string(), "&kp SHORT".to_string()],
                    layer_type: LayerType::Deflayer,
                    source_layer: None,
                    key_bindings: HashMap::new(),
                },
            ],
            includes: vec![],
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
        
        phantom_keys: vec![],
            chordsv2: vec![],
    };

        let result = generate_keymap_dts(content, &data).unwrap();
        // Layer 0: "&kp LONG_BINDING" (16) + " " + "&kp B"
        // Layer 1: "&kp A           " (16) + " " + "&kp SHORT"
        println!("Result:\n{}", result);
        assert!(result.contains("&kp LONG_BINDING &kp B"));
        assert!(result.contains("&kp A            &kp SHORT"));
    }

    #[test]
    fn test_generate_keymap_dts_addition() {
        let content = r#"
/ {
    keymap {
        compatible = "zmk,keymap";
        layer_0 {
            bindings = <&kp A &kp B>;
        };
    };
};
"#;
        let data = KeymapData {
            physical_layout: vec![
                PhysicalKey {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
                PhysicalKey {
                    x: 200,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
            ],
            layers: vec![
                Layer {
                    name: "layer_0".to_string(),
                    bindings: vec!["&kp A".to_string(), "&kp B".to_string()],
                    layer_type: LayerType::Deflayer,
                    source_layer: None,
                    key_bindings: HashMap::new(),
                },
                Layer {
                    name: "new_layer".to_string(),
                    bindings: vec!["&kp C".to_string(), "&kp D".to_string()],
                    layer_type: LayerType::Deflayer,
                    source_layer: None,
                    key_bindings: HashMap::new(),
                },
            ],
            includes: vec![],
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
        
        phantom_keys: vec![],
            chordsv2: vec![],
    };

        let result = generate_keymap_dts(content, &data).unwrap();
        println!("Result:\n{}", result);
        assert!(result.contains("layer_0"));
        assert!(result.contains("new_layer"));
        assert!(result.contains("&kp A &kp B"));
        assert!(result.contains("&kp C &kp D"));
        // Check that it's inside the keymap node (before the last closing brace)
        assert!(result.trim().ends_with("};"));
    }

    #[test]
    fn test_generate_kanata_surf() {
        let content = include_str!("../../static/kanata-surf.kbd");
        let data = parse_kanata_with_tree_sitter(content, false, false).unwrap();
        let result = generate_kanata_kbd(content, &data).unwrap();
        // The result should be identical if no changes were made
        assert_eq!(content, result);

        // Try changing a binding
        let mut modified_data = data.clone();
        modified_data.layers[0].bindings[0] = "MODIFIED".to_string();
        let result2 = generate_kanata_kbd(content, &modified_data).unwrap();
        assert!(result2.contains("MODIFIED"));
    }

    #[test]
    fn test_parse_kanata_surf() {
        let content = include_str!("../../static/kanata-surf.kbd");
        let result = parse_kanata_with_tree_sitter(content, false, false);
        match result {
            Ok(data) => {
                assert!(!data.layers.is_empty());
                assert!(!data.physical_layout.is_empty());

                // Row 0 (esc, f1, f2... del) -> 14 keys
                assert!(data.physical_layout.len() >= 14);
                assert_eq!(data.physical_layout[0].x, 0); // esc
                assert_eq!(data.physical_layout[1].x, 2000); // f1
                assert_eq!(data.physical_layout[2].x, 3000); // f2
                
                // Row 1 (grv, 1, 2...)
                let y1 = data.physical_layout[14].y;
                assert_eq!(y1, 1200); // Standard 108 row 1 y
                assert_eq!(data.physical_layout[14].x, 0); // grv
                assert_eq!(data.physical_layout[15].x, 1000); // 1
            }
            Err(e) => {
                panic!("Failed to parse kanata-surf.kbd: {}", e);
            }
        }
    }

    #[test]
    fn test_generate_keymap_includes_placement_multiple() {
        let content = r#"#include <behaviors.dtsi>
#include <dt-bindings/zmk/keys.h>

/ {
    keymap {
        compatible = "zmk,keymap";
        layer_0 {
            bindings = <&kp A &kp B>;
        };
    };
};
"#;
        let data = KeymapData {
            physical_layout: vec![
                PhysicalKey {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
                PhysicalKey {
                    x: 200,
                    y: 0,
                    width: 100,
                    height: 100,
                    rotation: 0,
                    rx: 0,
                    ry: 0,
                origin: KeyOrigin::Standard,
                name: String::new(),
            },
            ],
            layers: vec![Layer {
                name: "layer_0".to_string(),
                bindings: vec!["&mmv 0".to_string(), "&kp B".to_string()],
                layer_type: LayerType::Deflayer,
                source_layer: None,
                key_bindings: HashMap::new(),
            }],
            includes: vec!["custom.h".to_string()],
            aliases: HashMap::new(),
            defsrc: Vec::new(),
            unmapped_names: Vec::new(),
            process_unmapped_keys: ProcessUnmappedKeys::No,
            defvars: Vec::new(),
        
        phantom_keys: vec![],
            chordsv2: vec![],
    };

        let result = generate_keymap_dts(content, &data).unwrap();
        println!("Result:\n{}", result);

        // Should have added custom.h, mouse_move.dtsi, and pointing.h
        assert!(result.contains("#include <custom.h>"));
        assert!(result.contains("#include <behaviors/mouse_move.dtsi>"));
        assert!(result.contains("#include <dt-bindings/zmk/pointing.h>"));

        // Check placement: should be after keys.h
        let pos_keys = result.find("#include <dt-bindings/zmk/keys.h>").unwrap();
        let pos_custom = result.find("#include <custom.h>").unwrap();
        let pos_mmv = result.find("#include <behaviors/mouse_move.dtsi>").unwrap();
        let pos_pointing = result
            .find("#include <dt-bindings/zmk/pointing.h>")
            .unwrap();

        assert!(pos_custom > pos_keys, "custom.h should be after keys.h");
        assert!(pos_mmv > pos_keys, "mouse_move.dtsi should be after keys.h");
        assert!(pos_pointing > pos_keys, "pointing.h should be after keys.h");
        assert!(pos_mmv > pos_keys, "mouse_move.dtsi should be after keys.h");
    }

    #[test]
    fn test_svg_generation_hshs52() {
        use thockflow::keymap::generate_svg;
        let content = include_str!("../../static/hshs52.keymap");
        let data = parse_keymap_with_tree_sitter(content).expect("Should parse hshs52");

        let svg = generate_svg(&data, false, false, false);
        assert!(!svg.is_empty());
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));

        // Validate SVG using roxmltree
        let doc = match roxmltree::Document::parse(&svg) {
            Ok(d) => d,
            Err(e) => {
                let pos = e.pos();
                let start = (pos.col as usize).saturating_sub(50);
                let end = (svg.len()).min(pos.col as usize + 50);
                eprintln!("XML Error at col {}: {:?}", pos.col, e);
                eprintln!("Context: ...{}...", &svg[start..end]);
                panic!("Generated SVG should be valid XML");
            }
        };
        let root = doc.root_element();
        assert_eq!(root.tag_name().name(), "svg");

        // Check for layers (layer-title class)
        let layer_titles: Vec<_> = root
            .descendants()
            .filter(|n| n.attribute("class") == Some("layer-title"))
            .collect();
        assert_eq!(
            layer_titles.len(),
            data.layers.len(),
            "Should have correct number of layer titles"
        );

        // Check for keys (rect with class "key")
        let keys: Vec<_> = root
            .descendants()
            .filter(|n| n.attribute("class") == Some("key"))
            .collect();
        assert_eq!(
            keys.len(),
            data.layers.len() * data.physical_layout.len(),
            "Should have correct total number of keys across all layers"
        );

        println!(
            "SVG validation passed: {} layers, {} total keys",
            layer_titles.len(),
            keys.len()
        );
    }

    #[test]
    fn test_patch_keymap_endpoint() {
        // Test patching an existing keymap file with modified layer data
        let file_content = include_str!("../../static/hshs52.keymap");
        
        // First parse the original to get the data
        let mut data = parse_keymap_with_tree_sitter(file_content).expect("Should parse hshs52");
        
        // Modify a binding to simulate editing
        if !data.layers.is_empty() && !data.layers[0].bindings.is_empty() {
            data.layers[0].bindings[0] = "&kp X".to_string();
        }
        
        // Now patch the file with the modified data
        let result = generate_keymap_dts(file_content, &data).expect("Should generate patched keymap");
        
        // Validate the result
        let reparsed = parse_keymap_with_tree_sitter(&result);
        if let Err(ref e) = reparsed {
            eprintln!("Parse error: {}", e);
            // Print entire result for debugging
            eprintln!("=== GENERATED CONTENT START ===");
            eprintln!("{}", result);
            eprintln!("=== GENERATED CONTENT END ===");
        }
        assert!(reparsed.is_ok(), "Patched keymap should be valid: {:?}", reparsed.err());
        
        // Check the modification was applied
        assert!(result.contains("&kp X"), "Modified binding should be present");
    }
    
    #[test]
    fn test_patch_simple_keymap() {
        // Test with a minimal keymap that has a physical layout
        let file_content = r#"
/ {
    layout: layout {
        compatible = "zmk,physical-layout";
        keys = <&key_physical_attrs 100 100 0 0 0 0 0
                &key_physical_attrs 100 100 1000 0 0 0 0>;
    };
    
    keymap {
        compatible = "zmk,keymap";
        default_layer {
            bindings = <&kp A &kp B>;
        };
        lower {
            bindings = <&kp C &kp D>;
        };
    };
};
"#;
        
        let mut data = parse_keymap_with_tree_sitter(file_content).expect("Should parse simple");
        
        // Modify first binding
        if !data.layers.is_empty() && !data.layers[0].bindings.is_empty() {
            data.layers[0].bindings[0] = "&kp Z".to_string();
        }
        
        eprintln!("=== ORIGINAL ===");
        eprintln!("{}", file_content);
        eprintln!("=== LAYER DATA ===");
        for (i, layer) in data.layers.iter().enumerate() {
            eprintln!("Layer {}: {} with {} bindings", i, layer.name, layer.bindings.len());
            eprintln!("  Bindings: {:?}", layer.bindings);
        }
        
        let result = generate_keymap_dts(file_content, &data).expect("Should generate");
        
        eprintln!("=== RESULT ===");
        eprintln!("{}", result);
        
        // Check structure
        let reparsed = parse_keymap_with_tree_sitter(&result);
        assert!(reparsed.is_ok(), "Should reparse: {:?}", reparsed.err());
        assert!(result.contains("&kp Z"), "Should have Z");
    }

    #[test]
    fn test_bindings_value_node_format() {
        // Test to understand what tree-sitter considers the "value" node for bindings
        let content = r#"
/ {
    keymap {
        compatible = "zmk,keymap";
        default_layer {
            bindings = <&kp A &kp B>;
        };
    };
};
"#;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_devicetree::LANGUAGE.into()).unwrap();
        let tree = parser.parse(content, None).unwrap();
        
        fn find_bindings_property<'a>(node: tree_sitter::Node<'a>, source: &[u8]) -> Option<tree_sitter::Node<'a>> {
            if node.kind() == "property" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = name_node.utf8_text(source).unwrap_or("");
                    if name == "bindings" {
                        return Some(node);
                    }
                }
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(result) = find_bindings_property(child, source) {
                    return Some(result);
                }
            }
            None
        }
        
        if let Some(prop_node) = find_bindings_property(tree.root_node(), content.as_bytes()) {
            // Try child_by_field_name("value")
            if let Some(value_node) = prop_node.child_by_field_name("value") {
                let kind = value_node.kind();
                let text = value_node.utf8_text(content.as_bytes()).unwrap_or("");
                
                // The value node is "integer_cells" and includes < >
                assert_eq!(kind, "integer_cells");
                assert!(text.starts_with('<'), "Value should start with <");
                assert!(text.ends_with('>'), "Value should end with >");
            } else {
                panic!("No value field found");
            }
        } else {
            panic!("Could not find bindings property");
        }
    }

    #[test]
    fn test_parse_deflayermap_with_process_unmapped_keys_yes() {
        let content = r#"
(defcfg
  process-unmapped-keys yes
)

(defsrc
  esc f1 a b
)

(deflayer base
  esc f1 a b
)

(deflayermap (nav)
  a (layer-toggle symbols)
  b left
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Check process-unmapped-keys was parsed
        assert!(matches!(result.process_unmapped_keys, ProcessUnmappedKeys::Yes));

        // Should have 2 layers
        assert_eq!(result.layers.len(), 2, "Expected 2 layers");

        // Find the nav layer
        let nav_layer = result.layers.iter().find(|l| l.name == "nav").expect("Should have nav layer");

        // Check it's a Deflayermap
        assert!(matches!(nav_layer.layer_type, LayerType::Deflayermap));

        // Check key_bindings were stored
        assert_eq!(nav_layer.key_bindings.get("a"), Some(&"(layer-toggle symbols)".to_string()));
        assert_eq!(nav_layer.key_bindings.get("b"), Some(&"left".to_string()));

        // With process-unmapped-keys yes, unmapped keys should inherit from base
        // defsrc order: esc(0), f1(1), a(2), b(3)
        // nav mappings: a->(layer-toggle symbols), b->left
        // So nav bindings should be: ["esc", "f1", "(layer-toggle symbols)", "left"]
        assert_eq!(nav_layer.bindings[0], "esc", "esc should inherit from base");
        assert_eq!(nav_layer.bindings[1], "f1", "f1 should inherit from base");
        assert_eq!(nav_layer.bindings[2], "(layer-toggle symbols)", "a should have explicit mapping");
        assert_eq!(nav_layer.bindings[3], "left", "b should have explicit mapping");
    }

    #[test]
    fn test_parse_deflayermap_with_process_unmapped_keys_no() {
        let content = r#"
(defcfg
  process-unmapped-keys no
)

(defsrc
  esc f1 a b
)

(deflayer base
  esc f1 a b
)

(deflayermap (nav)
  a (layer-toggle symbols)
  b left
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Check process-unmapped-keys was parsed
        assert!(matches!(result.process_unmapped_keys, ProcessUnmappedKeys::No));

        // Find the nav layer
        let nav_layer = result.layers.iter().find(|l| l.name == "nav").expect("Should have nav layer");

        // With process-unmapped-keys no, unmapped keys should be transparent ("_")
        // nav bindings should be: ["_", "_", "(layer-toggle symbols)", "left"]
        assert_eq!(nav_layer.bindings[0], "_", "esc should be transparent");
        assert_eq!(nav_layer.bindings[1], "_", "f1 should be transparent");
        assert_eq!(nav_layer.bindings[2], "(layer-toggle symbols)", "a should have explicit mapping");
        assert_eq!(nav_layer.bindings[3], "left", "b should have explicit mapping");
    }

    #[test]
    fn test_parse_deflayermap_with_all_except() {
        let content = r#"
(defcfg
  process-unmapped-keys (all-except esc)
)

(defsrc
  esc f1 a b
)

(deflayer base
  esc f1 a b
)

(deflayermap (nav)
  b left
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Check process-unmapped-keys was parsed
        match &result.process_unmapped_keys {
            ProcessUnmappedKeys::AllExcept(exclude) => {
                assert_eq!(exclude, &["esc"]);
            }
            _ => panic!("Expected AllExcept variant"),
        }

        // Find the nav layer
        let nav_layer = result.layers.iter().find(|l| l.name == "nav").expect("Should have nav layer");

        // With (all-except esc), esc should be transparent, others should inherit from base
        // nav bindings should be: ["_", "f1", "a", "left"]
        assert_eq!(nav_layer.bindings[0], "_", "esc should be transparent (excluded)");
        assert_eq!(nav_layer.bindings[1], "f1", "f1 should inherit from base");
        assert_eq!(nav_layer.bindings[2], "a", "a should inherit from base");
        assert_eq!(nav_layer.bindings[3], "left", "b should have explicit mapping");
    }

    #[test]
    fn test_deflayermap_roundtrip() {
        // This test verifies that we can parse a file with deflayermap
        // and serialize it back while preserving the deflayermap structure
        let content = r#"(defcfg
  process-unmapped-keys yes
)

(defsrc
  esc f1 a b
)

(defalias
  cap1 (layer-toggle nav)
)

(deflayer base
  esc f1 a b
)

(deflayermap (nav)
  a (layer-toggle symbols)
  b left
)
"#;
        let data = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse");

        // Verify nav layer exists and is deflayermap
        let nav_layer = data.layers.iter().find(|l| l.name == "nav").expect("Should have nav layer");
        assert!(matches!(nav_layer.layer_type, LayerType::Deflayermap));

        // Generate output
        let result = generate_kanata_kbd(content, &data).expect("Should generate");

        // Verify the output still contains deflayermap
        assert!(result.contains("(deflayermap (nav)"), "Output should preserve deflayermap");
        assert!(result.contains("process-unmapped-keys yes"), "Output should preserve defcfg");
    }

    #[test]
    fn test_mixed_deflayer_and_deflayermap() {
        let content = r#"
(defcfg
  process-unmapped-keys yes
)

(defsrc a b c d)

(deflayer base _ _ _ _)

(deflayer full x y z w)

(deflayermap (partial)
  a x
  b y
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse");

        // Should have 3 layers
        assert_eq!(result.layers.len(), 3, "Expected 3 layers");

        // Check layer types
        let base_layer = result.layers.iter().find(|l| l.name == "base").expect("Should have base layer");
        let full_layer = result.layers.iter().find(|l| l.name == "full").expect("Should have full layer");
        let partial_layer = result.layers.iter().find(|l| l.name == "partial").expect("Should have partial layer");

        assert!(matches!(base_layer.layer_type, LayerType::Deflayer));
        assert!(matches!(full_layer.layer_type, LayerType::Deflayer));
        assert!(matches!(partial_layer.layer_type, LayerType::Deflayermap));

        // Check partial layer has correct explicit bindings
        assert_eq!(partial_layer.key_bindings.get("a"), Some(&"x".to_string()));
        assert_eq!(partial_layer.key_bindings.get("b"), Some(&"y".to_string()));
    }

    // ============================================================================
    // defvar Tests
    // ============================================================================

    #[test]
    fn test_parse_defvar_integer() {
        let content = r#"
(defvar tap-timeout 100)
(defvar hold-timeout 200)

(defsrc a b)

(deflayer base a b)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have 2 defvars
        assert_eq!(result.defvars.len(), 2, "Expected 2 defvars");

        // Check first variable
        assert_eq!(result.defvars[0].name, "tap-timeout");
        assert_eq!(result.defvars[0].value, "100");
        assert!(matches!(result.defvars[0].var_type, VarType::Integer));

        // Check second variable
        assert_eq!(result.defvars[1].name, "hold-timeout");
        assert_eq!(result.defvars[1].value, "200");
        assert!(matches!(result.defvars[1].var_type, VarType::Integer));
    }

    #[test]
    fn test_parse_defvar_key() {
        let content = r#"
(defvar my-mod lctl)
(defvar my-key a)

(defsrc a b)

(deflayer base $my-mod $my-key)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have 2 defvars
        assert_eq!(result.defvars.len(), 2, "Expected 2 defvars");

        // Check variable types
        let my_mod = result.defvars.iter().find(|v| v.name == "my-mod").expect("Should have my-mod");
        assert_eq!(my_mod.value, "lctl");
        assert!(matches!(my_mod.var_type, VarType::Key));

        let my_key = result.defvars.iter().find(|v| v.name == "my-key").expect("Should have my-key");
        assert_eq!(my_key.value, "a");
        assert!(matches!(my_key.var_type, VarType::Key));
    }

    #[test]
    fn test_parse_defvar_list() {
        let content = r#"
(defvar my-macro (a b c d))

(defsrc a b)

(deflayer base a b)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have 1 defvar
        assert_eq!(result.defvars.len(), 1, "Expected 1 defvar");

        assert_eq!(result.defvars[0].name, "my-macro");
        assert_eq!(result.defvars[0].value, "(a b c d)");
        assert!(matches!(result.defvars[0].var_type, VarType::List));
    }

    #[test]
    fn test_parse_defvar_action() {
        let content = r#"
(defvar nav-toggle (layer-toggle nav))

(defsrc a b)

(deflayer base a b)
(deflayer nav _ _)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have 1 defvar
        assert_eq!(result.defvars.len(), 1, "Expected 1 defvar");

        assert_eq!(result.defvars[0].name, "nav-toggle");
        assert_eq!(result.defvars[0].value, "(layer-toggle nav)");
        assert!(matches!(result.defvars[0].var_type, VarType::Action));
    }

    #[test]
    fn test_parse_defvar_mixed_types() {
        let content = r#"
(defvar timeout 100)
(defvar my-key lsft)
(defvar my-list (a b c))
(defvar my-action (tap-hold 200 200 a lctl))

(defsrc a b)

(deflayer base a b)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have 4 defvars
        assert_eq!(result.defvars.len(), 4, "Expected 4 defvars");

        // Check each type
        let timeout = result.defvars.iter().find(|v| v.name == "timeout").expect("Should have timeout");
        assert!(matches!(timeout.var_type, VarType::Integer));

        let my_key = result.defvars.iter().find(|v| v.name == "my-key").expect("Should have my-key");
        assert!(matches!(my_key.var_type, VarType::Key));

        let my_list = result.defvars.iter().find(|v| v.name == "my-list").expect("Should have my-list");
        assert!(matches!(my_list.var_type, VarType::List));

        let my_action = result.defvars.iter().find(|v| v.name == "my-action").expect("Should have my-action");
        assert!(matches!(my_action.var_type, VarType::Action));
    }

    #[test]
    fn test_detect_var_type_edge_cases() {
        // Test empty string
        assert!(matches!(detect_var_type(""), VarType::Unknown));
        assert!(matches!(detect_var_type("   "), VarType::Unknown));

        // Test negative integer
        assert!(matches!(detect_var_type("-100"), VarType::Integer));

        // Test string
        assert!(matches!(detect_var_type("\"hello\""), VarType::String));
        assert!(matches!(detect_var_type("'hello'"), VarType::String));

        // Test alias reference as key
        assert!(matches!(detect_var_type("@my-alias"), VarType::Key));
    }

    #[test]
    fn test_defvar_in_deflayermap() {
        let content = r#"
(defvar nav-toggle (layer-toggle nav))

(defsrc a b)

(deflayer base a b)

(deflayermap (nav)
  a $nav-toggle
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have 1 defvar
        assert_eq!(result.defvars.len(), 1, "Expected 1 defvar");
        assert_eq!(result.defvars[0].name, "nav-toggle");

        // Should have 2 layers
        assert_eq!(result.layers.len(), 2, "Expected 2 layers");
    }

    // ============================================================================
    // Phantom Key Tests
    // ============================================================================

    #[test]
    fn test_phantom_keys_computed_when_key_missing_from_defsrc() {
        let content = r#"
(defsrc
  f1   f2   f3
)

(deflayer base
  f1   f2   f3
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // Should have phantom keys (esc and many others are not in defsrc)
        assert!(!result.phantom_keys.is_empty(), "Expected phantom keys when defsrc is missing standard keys");
        
        // Check that esc is a phantom key
        let has_esc_phantom = result.phantom_keys.iter().any(|p| p.name == "esc");
        assert!(has_esc_phantom, "Expected 'esc' to be a phantom key");
        
        // Physical layout should include both standard and phantom keys
        let phantom_count = result.physical_layout.iter()
            .filter(|pk| pk.origin == KeyOrigin::Phantom)
            .count();
        assert!(phantom_count > 0, "Expected phantom keys in physical_layout");
        
        // Check that 'esc' is in physical_layout as a phantom
        let esc_phantom = result.physical_layout.iter()
            .find(|pk| pk.name == "esc" && pk.origin == KeyOrigin::Phantom);
        assert!(esc_phantom.is_some(), "Expected 'esc' phantom key in physical_layout");
    }

    #[test]
    fn test_no_phantom_keys_when_all_keys_present() {
        let content = r#"
(defsrc
  esc  f1   f2   f3   f4   f5   f6   f7   f8   f9   f10  f11  f12
)

(deflayer base
  esc  f1   f2   f3   f4   f5   f6   f7   f8   f9   f10  f11  f12
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse successfully");

        // These keys are in defsrc, so they shouldn't be phantoms
        let has_f1_phantom = result.phantom_keys.iter().any(|p| p.name == "f1");
        assert!(!has_f1_phantom, "Expected 'f1' NOT to be a phantom key since it's in defsrc");
    }

    #[test]
    fn test_phantom_keys_repro() {
        let content = r#"
(defsrc
  a b c
)

(deflayer base
  a b c
)
"#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse");
        assert!(result.phantom_keys.len() > 100);
    }
        
    #[test]
    fn test_phantom_keys_serialization() {
        let content = r#"
    (defsrc
    a
    )

    (deflayer base
    a
    )
    "#;
        let result = parse_kanata_with_tree_sitter(content, false, false).expect("Should parse");
        let json = serde_json::to_string(&result).expect("Should serialize");

        println!("JSON OUTPUT:\n{}", json);

        // Check if "phantom_keys" field exists and is not empty
        assert!(json.contains("\"phantom_keys\":[{"), "JSON should contain phantom_keys array");        
        // Check if "origin":"Phantom" exists
        assert!(json.contains("\"origin\":\"Phantom\""), "JSON should contain Phantom origin");
        
        // Check if physical_layout has many items
        assert!(result.physical_layout.len() > 100);
    }

    #[test]
    fn test_phantom_key_addition() {
        let content = r#"
(defsrc
  a b c
)

(deflayer base
  a b c
)
"#;
        let mut data = parse_kanata_with_tree_sitter(content, false, false).unwrap();
        
        let phantom = data.phantom_keys.iter().find(|p| p.name == "esc").unwrap().clone();
        
        data.defsrc.push(phantom.name.clone());
        let insert_idx = data.defsrc.len() - 1;
        data.layers[0].bindings.insert(insert_idx, "esc".to_string());
        
        let new_content = generate_kanata_kbd(content, &data).unwrap();
        
        let new_data = parse_kanata_with_tree_sitter(&new_content, false, false).unwrap();
        assert!(new_data.defsrc.contains(&"esc".to_string()));
        assert!(new_data.layers[0].bindings.contains(&"esc".to_string()));
    }
}

static LOCAL_POOL: Lazy<LocalPoolHandle> = Lazy::new(|| LocalPoolHandle::new(num_cpus::get()));

fn html_wasm_init_head(init_quote_index: usize) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        r#"
    <script>window.THOCKFLOW_INIT_INDEX = {};</script>
    <script type="module">
      import init from "{js_path}?v={ts}";
      init({{ module_or_path: "{wasm_path}?v={ts}" }});
    </script>
"#,
        init_quote_index,
        js_path = *APP_JS_PATH,
        wasm_path = *APP_WASM_PATH,
        ts = timestamp,
    )
}

async fn index(
    Extension(index_html_s): Extension<String>,
    url: Request<Body>,
    Query(queries): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let init_quote_index = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        (now % 10000) as usize // Simple pseudo-random using nanoseconds
    };

    let out = LOCAL_POOL
        .spawn_pinned(move || async move {
            let props = ServerAppProps {
                path: url.uri().path().to_owned().into(),
                queries,
                init_quote_index: Some(init_quote_index),
            };
            let mut out = String::new();
            yew::ServerRenderer::<thockflow::ServerApp>::with_props(move || props)
                .render_to_string(&mut out)
                .await;
            out
        })
        .await
        .unwrap();
    // Remove dev script tag if present to avoid duplicate loads
    let html = index_html_s
        .replace("<body>", &format!("<body>{}", out))
        .replace(
            "</head>",
            &format!("{}</head>", html_wasm_init_head(init_quote_index)),
        );
    (
        HeaderMap::from_iter([(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))]),
        Html(html),
    )
}

async fn handle_error(e: impl std::fmt::Debug) -> impl IntoResponse {
    eprintln!("{e:?}");
    StatusCode::BAD_REQUEST
}

fn find_kanata_node<'a>(
    node: tree_sitter::Node<'a>,
    source: &[u8],
    kind_name: &str,
) -> Vec<tree_sitter::Node<'a>> {
    let mut results = Vec::new();
    let mut found = false;
    if node.kind() == "list" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "symbol" || kind == "boolean" || kind == "number" {
                if child.utf8_text(source).unwrap_or("") == kind_name {
                    results.push(node);
                    found = true;
                }
                break;
            }
        }
    }
    if !found {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            results.extend(find_kanata_node(child, source, kind_name));
        }
    }
    results
}

/// Parse defcfg to extract process-unmapped-keys setting
fn parse_defcfg(node: tree_sitter::Node, source: &[u8]) -> ProcessUnmappedKeys {
    let mut result = ProcessUnmappedKeys::No;
    let mut cursor = node.walk();
    let mut prev_was_process_unmapped_keys = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        let text = child.utf8_text(source).unwrap_or("");

        if prev_was_process_unmapped_keys {
            // This child is the value for process-unmapped-keys
            if text == "yes" {
                result = ProcessUnmappedKeys::Yes;
            } else if text == "no" {
                result = ProcessUnmappedKeys::No;
            } else if kind == "list" {
                // Check for (all-except ...)
                let mut inner_cursor = child.walk();
                let mut found_all_except = false;
                let mut excluded_keys = Vec::new();

                for inner_child in child.children(&mut inner_cursor) {
                    let inner_text = inner_child.utf8_text(source).unwrap_or("");
                    if inner_text == "all-except" {
                        found_all_except = true;
                    } else if found_all_except && (inner_child.kind() == "symbol" || inner_child.kind() == "boolean") {
                        excluded_keys.push(inner_text.to_string());
                    }
                }

                if found_all_except {
                    result = ProcessUnmappedKeys::AllExcept(excluded_keys);
                }
            }
            prev_was_process_unmapped_keys = false;
        } else if kind == "symbol" && text == "process-unmapped-keys" {
            prev_was_process_unmapped_keys = true;
        }
    }

    result
}

/// Parse a deflayermap node and return a Layer
/// Syntax: (deflayermap (layer-name) key1 action1 key2 action2 ...)
fn parse_deflayermap(
    node: tree_sitter::Node,
    source: &[u8],
    defsrc_keys: &[String],
    base_layer_bindings: Option<&[String]>,
    process_unmapped: &ProcessUnmappedKeys,
) -> Option<Layer> {
    let mut cursor = node.walk();
    let mut children: Vec<tree_sitter::Node> = Vec::new();

    // Collect all children first (as nodes, not text), skipping parentheses
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        // Skip parentheses - only keep actual content
        if kind != "(" && kind != ")" {
            children.push(child);
        }
    }

    if children.len() < 2 {
        return None;
    }

    // First child should be the symbol "deflayermap"
    let first_text = children[0].utf8_text(source).unwrap_or("");
    if first_text != "deflayermap" {
        return None;
    }

    // Second child should be a list containing the layer name: (layer-name)
    let layer_name = if children[1].kind() == "list" {
        // Extract layer name from inside the parentheses
        let mut name_cursor = children[1].walk();
        let mut name = String::new();
        for child in children[1].children(&mut name_cursor) {
            let kind = child.kind();
            if kind != "(" && kind != ")" {
                name = child.utf8_text(source).unwrap_or("").to_string();
                break;
            }
        }
        name
    } else {
        // Fallback: try direct text (for backwards compatibility if syntax varies)
        children[1].utf8_text(source).unwrap_or("").to_string()
    };
    
    if layer_name.is_empty() {
        return None;
    }

    // Parse key-action pairs (starting from index 2)
    let mut key_bindings: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut i = 2;
    while i + 1 < children.len() {
        let key_node = children[i];
        let action_node = children[i + 1];

        let key_kind = key_node.kind();
        let key_name = key_node.utf8_text(source).unwrap_or("").to_string();

        // Only process if key is a symbol/boolean and is in defsrc
        if (key_kind == "symbol" || key_kind == "boolean") && defsrc_keys.contains(&key_name) {
            let action_text = action_node.utf8_text(source).unwrap_or("").to_string();
            key_bindings.insert(key_name, action_text);
        }

        i += 2;
    }

    // Build full bindings vector based on defsrc order
    let mut bindings = Vec::new();

    for (idx, key_name) in defsrc_keys.iter().enumerate() {
        if let Some(action) = key_bindings.get(key_name) {
            // Explicit mapping in deflayermap
            bindings.push(action.clone());
        } else {
            // No explicit mapping - determine default behavior
            match process_unmapped {
                ProcessUnmappedKeys::Yes => {
                    // Key is available with its base action
                    if let Some(base_bindings) = base_layer_bindings {
                        bindings.push(base_bindings.get(idx).cloned().unwrap_or_else(|| "_".to_string()));
                    } else {
                        bindings.push(key_name.clone());
                    }
                }
                ProcessUnmappedKeys::No => {
                    // Key is not processed (transparent)
                    bindings.push("_".to_string());
                }
                ProcessUnmappedKeys::AllExcept(exclude) => {
                    if exclude.contains(key_name) {
                        bindings.push("_".to_string());
                    } else if let Some(base_bindings) = base_layer_bindings {
                        bindings.push(base_bindings.get(idx).cloned().unwrap_or_else(|| "_".to_string()));
                    } else {
                        bindings.push(key_name.clone());
                    }
                }
            }
        }
    }

    Some(Layer {
        name: layer_name,
        bindings,
        layer_type: LayerType::Deflayermap,
        source_layer: Some("defsrc".to_string()),
        key_bindings,
    })
}

/// Detect the type of a variable value based on its content
fn detect_var_type(value: &str) -> VarType {
    let trimmed = value.trim();
    
    // Empty or whitespace-only
    if trimmed.is_empty() {
        return VarType::Unknown;
    }
    
    // Integer: pure digits, optionally with leading minus sign
    if trimmed.parse::<i64>().is_ok() {
        return VarType::Integer;
    }
    
    // String: wrapped in quotes
    if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
       (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
        return VarType::String;
    }
    
    // List: wrapped in parentheses with multiple space-separated items
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len()-1];
        let parts: Vec<&str> = inner.split_whitespace().collect();
        if parts.len() > 1 {
            // Check if it looks like an action (first part is an action name)
            let action_names = ["tap-hold", "tap-hold-press", "tap-hold-release", "tap-hold-next",
                "tap-hold-next-release", "layer-toggle", "layer-switch", "layer-while-held",
                "macro", "multi", "one-shot", "tap-dance", "caps-word", "unicode"];
            if parts.len() > 0 && action_names.contains(&parts[0]) {
                return VarType::Action;
            }
            return VarType::List;
        }
        // Single item in parens might be a grouped expression
        return VarType::Action;
    }
    
    // Check if it's a single key (alphanumeric, key name, or alias reference)
    // Simple heuristic: single word without special characters
    if trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '@') {
        // If it starts with @, it's an alias reference (treat as Key type)
        if trimmed.starts_with('@') {
            return VarType::Key;
        }
        // Single key names are typically short
        if trimmed.len() <= 10 {
            return VarType::Key;
        }
    }
    
    VarType::Unknown
}

/// Compute phantom keys - keys in the physical layout but not in defsrc
fn compute_phantom_keys(
    defsrc: &[String],
    is_mac: bool,
    is_laptop: bool,
) -> Vec<PhantomKey> {
    use thockflow::kanata::layout;
    
    // Get the appropriate layout
    let layout_map = match (is_mac, is_laptop) {
        (true, true) => &*layout::MACBOOK_LAYOUT,
        (true, false) => &*layout::MAC_LAYOUT,
        (false, true) => &*layout::WIN_LAPTOP_LAYOUT,
        (false, false) => &*layout::STANDARD_108_LAYOUT,
    };

    let defsrc_set: HashSet<_> = defsrc.iter().map(|s| s.to_lowercase()).collect();
    let mut defsrc_positions = HashSet::new();
    for name in &defsrc_set {
        if let Some(&(x, y)) = layout_map.get(name.as_str()) {
            defsrc_positions.insert((x, y));
        }
    }
    let mut phantom_keys = Vec::new();
    let mut used_positions = HashSet::new();

    for (name, &(x, y)) in layout_map.iter() {
        // Skip single-character aliases like ".", ",", etc. - they're duplicates
        if name.len() == 1 && !name.chars().next().unwrap().is_alphanumeric() {
            continue;
        }
        
        // Skip if this position is already occupied by a standard key
        if defsrc_positions.contains(&(x, y)) {
            continue;
        }
        
        // Skip if we already added a phantom key at this position
        if used_positions.contains(&(x, y)) {
            continue;
        }
        
        // Check if this key is already in defsrc
        if !defsrc_set.contains(name.to_lowercase().as_str()) {
            used_positions.insert((x, y));
            phantom_keys.push(PhantomKey {
                name: name.to_string(),
                position: (x, y),
            });
        }
    }

    phantom_keys
}

/// Compute physical layout including phantom keys at their proper positions
fn compute_physical_layout_with_phantoms(
    key_names: &[String],
    unmapped_names: &[String],
    phantom_keys: &[PhantomKey],
    alias_names: &[String],
    is_mac: bool,
    is_laptop: bool,
) -> Vec<PhysicalKey> {
    use thockflow::kanata::layout;
    
    let layout_map = match (is_mac, is_laptop) {
        (true, true) => &*layout::MACBOOK_LAYOUT,
        (true, false) => &*layout::MAC_LAYOUT,
        (false, true) => &*layout::WIN_LAPTOP_LAYOUT,
        (false, false) => &*layout::STANDARD_108_LAYOUT,
    };

    let mut layout = Vec::new();
    let key_width = 1000;
    let key_height = 1000;

    // 1. Process standard keys from defsrc (at their proper positions, in defsrc order)
    for name in key_names {
        if let Some(&(x, y)) = layout_map.get(name.to_lowercase().as_str()) {
            layout.push(PhysicalKey {
                x,
                y,
                width: key_width,
                height: key_height,
                rotation: 0,
                rx: 0,
                ry: 0,
                origin: KeyOrigin::Standard,
                name: name.clone(),
            });
        }
    }

    // 2. Process phantom keys at their proper positions
    // Phantom keys are sorted by Y then X to maintain visual layout
    let mut phantom_layout: Vec<PhysicalKey> = phantom_keys.iter().map(|phantom| {
        PhysicalKey {
            x: phantom.position.0,
            y: phantom.position.1,
            width: key_width,
            height: key_height,
            rotation: 0,
            rx: 0,
            ry: 0,
            origin: KeyOrigin::Phantom,
            name: phantom.name.clone(),
        }
    }).collect();
    
    // Sort phantom keys by Y then X for visual ordering
    phantom_layout.sort_by(|a, b| {
        let y_diff = (a.y / 10).cmp(&(b.y / 10));
        if y_diff == std::cmp::Ordering::Equal {
            a.x.cmp(&b.x)
        } else {
            y_diff
        }
    });
    
    layout.extend(phantom_layout);

    // 3. Process unmapped keys at the bottom
    let unmapped_y_start = 6500;
    let unmapped_margin = 100;
    let unmapped_cols = 10;

    for (i, name) in unmapped_names.iter().enumerate() {
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
            origin: KeyOrigin::Unmapped,
            name: name.clone(),
        });
    }

    // 4. Process aliases at the bottom (below unmapped)
    let alias_y_start = 8000;
    let alias_margin = 100;
    let alias_cols = 10;
    
    for (i, name) in alias_names.iter().enumerate() {
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
            origin: KeyOrigin::Alias,
            name: name.clone(),
        });
    }

    layout
}

fn parse_kanata_with_tree_sitter(content: &str, is_mac: bool, is_laptop: bool) -> Result<KeymapData> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_scheme::LANGUAGE.into())?;
    let tree = parser
        .parse(content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse KBD"))?;

    let source = content.as_bytes();
    let root = tree.root_node();

    if root.has_error() {
        let mut error_pos = String::new();
        fn find_error(node: tree_sitter::Node, source: &[u8], pos: &mut String) {
            if node.has_error() {
                if node.kind() == "ERROR" {
                    *pos = format!(
                        "Tree-sitter parse error at line {}, column {}",
                        node.start_position().row + 1,
                        node.start_position().column + 1
                    );
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        find_error(child, source, pos);
                        if !pos.is_empty() {
                            return;
                        }
                    }
                }
            }
        }
        find_error(root, source, &mut error_pos);
        if !error_pos.is_empty() {
            return Err(anyhow::anyhow!(error_pos));
        }
    }

    // Parse defcfg for process-unmapped-keys
    let defcfg_nodes = find_kanata_node(root, source, "defcfg");
    let mut process_unmapped_keys = ProcessUnmappedKeys::No;
    for cfg_node in defcfg_nodes {
        process_unmapped_keys = parse_defcfg(cfg_node, source);
    }

    let defsrc_nodes = find_kanata_node(root, source, "defsrc");
    if defsrc_nodes.is_empty() {
        return Err(anyhow::anyhow!("Missing (defsrc ...)"));
    }
    let defsrc = defsrc_nodes[0];

    fn collect_keys<'a>(node: tree_sitter::Node<'a>, source: &'a [u8], keys_raw: &mut Vec<tree_sitter::Node<'a>>) {
        let kind = node.kind();
        if kind == "symbol" || kind == "boolean" || kind == "number" {
            let text = node.utf8_text(source).unwrap_or("");
            if text != "defsrc" && text != "deflayer" {
                keys_raw.push(node);
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_keys(child, source, keys_raw);
        }
    }

    let mut keys_raw = Vec::new();
    let mut cursor = defsrc.walk();
    for child in defsrc.children(&mut cursor) {
        collect_keys(child, source, &mut keys_raw);
    }

    let defalias_nodes = find_kanata_node(root, source, "defalias");
    let mut aliases = std::collections::HashMap::new();
    for alias_node in defalias_nodes {
        let mut inner_cursor = alias_node.walk();
        let mut first = true;
        let mut last_name = String::new();

        for child in alias_node.children(&mut inner_cursor) {
            let kind = child.kind();
            if kind == "symbol" || kind == "boolean" || kind == "number" || kind == "list" {
                let text = child.utf8_text(source).unwrap_or("").to_string();
                if first && text == "defalias" {
                    first = false;
                    continue;
                }
                if last_name.is_empty() {
                    last_name = text;
                } else {
                    aliases.insert(last_name.clone(), text);
                    last_name = String::new();
                }
            }
        }
    }

    // Parse defvar nodes
    let defvar_nodes = find_kanata_node(root, source, "defvar");
    let mut defvars = Vec::new();
    for defvar_node in defvar_nodes {
        let mut inner_cursor = defvar_node.walk();
        let mut first = true;
        let mut last_name = String::new();

        for child in defvar_node.children(&mut inner_cursor) {
            let kind = child.kind();
            if kind == "symbol" || kind == "boolean" || kind == "number" || kind == "list" {
                let text = child.utf8_text(source).unwrap_or("").to_string();
                if first && text == "defvar" {
                    first = false;
                    continue;
                }
                if last_name.is_empty() {
                    last_name = text;
                } else {
                    let var_type = detect_var_type(&text);
                    defvars.push(Defvar {
                        name: last_name.clone(),
                        value: text,
                        var_type,
                    });
                    last_name = String::new();
                }
            }
        }
    }

    let mut filtered_indices = Vec::new();
    let mut key_names = Vec::new();
    let mut unmapped_names = Vec::new();
    let mut unmapped_indices = Vec::new();

    for (i, node) in keys_raw.iter().enumerate() {
        let name = node.utf8_text(source).unwrap_or("").to_string();
        if thockflow::kanata::layout::is_standard_key(&name, is_mac, is_laptop) {
            key_names.push(name);
            filtered_indices.push(i);
        } else {
            unmapped_names.push(name);
            unmapped_indices.push(i);
        }
    }

    let mut sorted_alias_names: Vec<String> = aliases.keys().cloned().collect();
    sorted_alias_names.sort();

    // Compute phantom keys (keys in physical layout but not in defsrc)
    // We always compute phantom keys so users can visually discover and add them to their config
    let should_show_phantoms = true;
    println!("DEBUG: should_show_phantoms={}, process_unmapped_keys={:?}", should_show_phantoms, process_unmapped_keys);
    let phantom_keys = if should_show_phantoms {
        let keys = compute_phantom_keys(&key_names, is_mac, is_laptop);
        println!("DEBUG: compute_phantom_keys returned {} keys", keys.len());
        keys
    } else {
        vec![]
    };

    // Compute physical layout including phantom keys at their proper positions
    let physical_layout = compute_physical_layout_with_phantoms(
        &key_names, 
        &unmapped_names, 
        &phantom_keys,
        &sorted_alias_names, 
        is_mac, 
        is_laptop
    );


    fn collect_bindings<'a>(node: tree_sitter::Node<'a>, source: &'a [u8], bindings: &mut Vec<String>, is_first: &mut bool, layer_name: &mut String) {
        let kind = node.kind();
        if kind == "symbol" || kind == "boolean" || kind == "number" {
            let text = node.utf8_text(source).unwrap_or("").to_string();
            if *is_first && text == "deflayer" {
                *is_first = false;
                return;
            }
            if layer_name.is_empty() {
                *layer_name = text;
            } else {
                bindings.push(text);
            }
        } else if kind == "list" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_bindings(child, source, bindings, is_first, layer_name);
            }
        }
    }

    let deflayer_nodes = find_kanata_node(root, source, "deflayer");
    let mut layers = Vec::new();
    for layer_node in deflayer_nodes {
        let mut inner_cursor = layer_node.walk();
        let mut layer_name = String::new();
        let mut raw_bindings = Vec::new();
        let mut is_first = true;

        for child in layer_node.children(&mut inner_cursor) {
            collect_bindings(child, source, &mut raw_bindings, &mut is_first, &mut layer_name);
        }
        if !raw_bindings.is_empty() {
            // Build bindings in the order of physical_layout
            let mut bindings = Vec::new();
            
            // Track how many of each type we've added
            let mut standard_count = 0;
            let mut unmapped_count = 0;
            
            for pk in &physical_layout {
                match pk.origin {
                    KeyOrigin::Standard => {
                        // Get binding from raw_bindings using filtered_indices
                        if let Some(&raw_idx) = filtered_indices.get(standard_count) {
                            bindings.push(raw_bindings.get(raw_idx).cloned().unwrap_or_else(|| "_".to_string()));
                        } else {
                            bindings.push("_".to_string());
                        }
                        standard_count += 1;
                    }
                    KeyOrigin::Unmapped => {
                        // Get binding from raw_bindings using unmapped_indices
                        if let Some(&raw_idx) = unmapped_indices.get(unmapped_count) {
                            bindings.push(raw_bindings.get(raw_idx).cloned().unwrap_or_else(|| "_".to_string()));
                        } else {
                            bindings.push("_".to_string());
                        }
                        unmapped_count += 1;
                    }
                    KeyOrigin::Phantom => {
                        // Phantom keys start with transparent binding
                        bindings.push("_".to_string());
                    }
                    KeyOrigin::Alias => {
                        // Aliases are just the alias name itself (not from raw_bindings)
                        // These are added at the end
                    }
                }
            }
            
            // Add alias bindings at the end
            for alias_name in &sorted_alias_names {
                bindings.push(alias_name.clone());
            }
            
            layers.push(Layer {
                name: layer_name,
                bindings,
                layer_type: LayerType::Deflayer,
                source_layer: None,
                key_bindings: std::collections::HashMap::new(),
            });
        }
    }

    // Parse deflayermap nodes
    let deflayermap_nodes = find_kanata_node(root, source, "deflayermap");
    
    // Clone base bindings to avoid borrow checker issues
    let base_bindings: Vec<String> = layers.first().map(|l| l.bindings.clone()).unwrap_or_default();

    for map_node in deflayermap_nodes {
        if let Some(layer) = parse_deflayermap(map_node, source, &key_names, Some(&base_bindings), &process_unmapped_keys) {
            // Build bindings in the order of physical_layout (like we do for deflayer)
            let mut full_bindings = Vec::new();
            
            // Track position in layer.bindings (which is ordered by key_names)
            let key_names_lower: Vec<String> = key_names.iter().map(|s| s.to_lowercase()).collect();
            
            for pk in &physical_layout {
                match pk.origin {
                    KeyOrigin::Standard => {
                        // Find index in key_names
                        if let Some(idx) = key_names_lower.iter().position(|n| n == &pk.name.to_lowercase()) {
                            full_bindings.push(layer.bindings.get(idx).cloned().unwrap_or_else(|| "_".to_string()));
                        } else {
                            full_bindings.push("_".to_string());
                        }
                    }
                    KeyOrigin::Unmapped => {
                        // Unmapped keys come after standard in the original deflayermap bindings
                        // Find position in unmapped_names
                        // For now, use "_" as default
                        full_bindings.push("_".to_string());
                    }
                    KeyOrigin::Phantom => {
                        // Phantom keys start with transparent binding
                        full_bindings.push("_".to_string());
                    }
                    KeyOrigin::Alias => {
                        // Aliases added at end
                    }
                }
            }
            
            // Add alias bindings at the end
            for alias_name in &sorted_alias_names {
                full_bindings.push(alias_name.clone());
            }

            layers.push(Layer {
                name: layer.name,
                bindings: full_bindings,
                layer_type: layer.layer_type,
                source_layer: layer.source_layer,
                key_bindings: layer.key_bindings,
            });
        }
    }

    Ok(KeymapData {
        physical_layout,
        layers,
        includes: Vec::new(),
        aliases,
        defsrc: key_names,
        unmapped_names,
        process_unmapped_keys,
        defvars,
        phantom_keys,
        chordsv2: Vec::new(),
    })
}

fn generate_kanata_kbd(original: &str, data: &KeymapData) -> Result<String> {
    struct Replacement {
        start: usize,
        end: usize,
        text: String,
    }

    fn collect_keys<'a>(
        node: tree_sitter::Node<'a>,
        source: &[u8],
        keys: &mut Vec<String>,
        is_first: &mut bool,
    ) {
        if node.kind() == "symbol" || node.kind() == "boolean" || node.kind() == "number" {
            let text = node.utf8_text(source).unwrap_or("").to_string();
            if *is_first && text == "defsrc" {
                *is_first = false;
                return;
            }
            keys.push(text.to_lowercase());
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_keys(child, source, keys, is_first);
        }
    }

    fn update_bindings<'a>(
        node: tree_sitter::Node<'a>,
        source: &[u8],
        target_bindings: &[String],
        binding_idx: &mut usize,
        num_standard_keys: usize,
        replacements: &mut Vec<Replacement>,
    ) {
        let kind = node.kind();
        if kind == "symbol" || kind == "boolean" || kind == "number" || kind == "list" {
            let text = node.utf8_text(source).unwrap_or("");
            if text == "deflayer" || text == "base" {
                // Skip deflayer and layer name
                // Actually we need to skip the first two symbols of deflayer (deflayer + name)
                // This recursion makes it tricky.
                return;
            }

            // If it's a list, we need to check if it's an action or just a group of keys
            // In Kanata, (action ...) is a single binding.
            // But [...] or other lists might be just grouping.
            // Tree-sitter-scheme might be grouping [ ] as a list.

            if kind == "list" {
                // Check if this list is a single action (starts with '(')
                if text.starts_with('(') {
                    if *binding_idx < num_standard_keys {
                        if let Some(new_binding) = target_bindings.get(*binding_idx) {
                            if text != new_binding {
                                replacements.push(Replacement {
                                    start: node.start_byte(),
                                    end: node.end_byte(),
                                    text: new_binding.clone(),
                                });
                            }
                        }
                    }
                    *binding_idx += 1;
                    return;
                }

                // If it's not a single action action, recurse into it
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    update_bindings(
                        child,
                        source,
                        target_bindings,
                        binding_idx,
                        num_standard_keys,
                        replacements,
                    );
                }
                return;
            }

            // It's a single symbol/number/boolean binding
            if *binding_idx < num_standard_keys {
                if let Some(new_binding) = target_bindings.get(*binding_idx) {
                    if text != new_binding {
                        replacements.push(Replacement {
                            start: node.start_byte(),
                            end: node.end_byte(),
                            text: new_binding.clone(),
                        });
                    }
                }
            }
            *binding_idx += 1;
        }
    }

    let mut content = original.to_string();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_scheme::LANGUAGE.into())?;
    let tree = parser
        .parse(content.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse KBD"))?;
    let root = tree.root_node();
    let source = content.as_bytes();

    let defsrc_nodes = find_kanata_node(root, source, "defsrc");

    // 1. Identify new keys in defsrc (phantom key additions)
    let original_defsrc_keys = if let Some(defsrc_node) = defsrc_nodes.first() {
        let mut keys = Vec::new();
        let mut is_first = true;
        collect_keys(*defsrc_node, source, &mut keys, &mut is_first);
        keys
    } else {
        Vec::new()
    };

    let original_defsrc_set: HashSet<_> = original_defsrc_keys.iter().cloned().collect();
    
    // Add new keys to content using the specialized helper functions
    // We do this BEFORE other replacements because it modifies the string in-place
    // and we need to re-parse or adjust offsets if we want to combine them.
    // Actually, it's safer to do it first and then re-parse for the other replacements.
    
    let mut modified_content = original.to_string();
    let mut added_any = false;
    
    // Check for is_mac and is_laptop from the data or some other way.
    // KeymapData doesn't seem to store these.
    // However, the parse_kanata_api takes them.
    // For now, let's assume false/false or try to detect from content if possible.
    // In many cases we don't have them here. 
    // Let's use false, false as defaults, which is standard 108.
    let is_mac = false; 
    let is_laptop = false;

    for key_name in &data.defsrc {
        if !original_defsrc_set.contains(&key_name.to_lowercase()) {
            println!("DEBUG [generate_kanata_kbd]: Found new key to insert: {}", key_name);
            // New key found!
            match insert_key_into_defsrc(&mut modified_content, key_name, is_mac, is_laptop) {
                Ok(_) => {
                    println!("DEBUG [generate_kanata_kbd]: Successfully inserted {} into defsrc", key_name);
                    // Now we need to find the index where it was inserted in defsrc to update layers
                    // Re-parse to get the new defsrc order
                    let mut parser = tree_sitter::Parser::new();
                    parser.set_language(&tree_sitter_scheme::LANGUAGE.into()).unwrap();
                    let new_tree = parser.parse(modified_content.as_bytes(), None).unwrap();
                    let new_root = new_tree.root_node();
                    let new_defsrc_nodes = find_kanata_node(new_root, modified_content.as_bytes(), "defsrc");
                    
                    if let Some(defsrc_node) = new_defsrc_nodes.first() {
                        let mut new_keys = Vec::new();
                        let mut is_first = true;
                        collect_keys(*defsrc_node, modified_content.as_bytes(), &mut new_keys, &mut is_first);
                        
                        println!("DEBUG [generate_kanata_kbd]: Re-parsed defsrc, new_keys length: {}", new_keys.len());
                        if let Some(idx) = new_keys.iter().position(|k| k == &key_name.to_lowercase()) {
                            println!("DEBUG [generate_kanata_kbd]: Adding binding slot '_' to all layers for key {} at index {}", key_name, idx);
                            match add_binding_to_all_layers(&mut modified_content, "_", idx + 1) {
                                Ok(_) => println!("DEBUG [generate_kanata_kbd]: Successfully added '_' bindings to layers"),
                                Err(e) => println!("DEBUG [generate_kanata_kbd]: Failed to add bindings to layers: {}", e),
                            }
                            added_any = true;
                        } else {
                            println!("DEBUG [generate_kanata_kbd]: Failed to find newly inserted key {} in parsed defsrc!", key_name);
                        }
                    } else {
                        println!("DEBUG [generate_kanata_kbd]: Failed to find defsrc node after insertion!");
                    }
                }
                Err(e) => {
                    println!("DEBUG [generate_kanata_kbd]: Failed to insert key into defsrc: {}", e);
                }
            }
        }
    }

    // If we added keys, we must re-parse the modified_content for subsequent replacements
    if added_any {
        content = modified_content;
    }

    let tree = parser
        .parse(content.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("Failed to re-parse KBD"))?;
    let root = tree.root_node();
    let source = content.as_bytes();

    let deflayer_nodes = find_kanata_node(root, source, "deflayer");
    let defalias_nodes = find_kanata_node(root, source, "defalias");
    let deflayermap_nodes = find_kanata_node(root, source, "deflayermap");

    let mut replacements = Vec::new();

    // Handle defalias
    for alias_node in defalias_nodes {
        let mut inner_cursor = alias_node.walk();
        let mut first = true;
        let mut last_name = String::new();

        for child in alias_node.children(&mut inner_cursor) {
            let kind = child.kind();
            if kind == "symbol" || kind == "boolean" || kind == "number" || kind == "list" {
                let text = child.utf8_text(source).unwrap_or("").to_string();
                if first && text == "defalias" {
                    first = false;
                    continue;
                }
                if last_name.is_empty() {
                    last_name = text;
                } else {
                    if let Some(new_val) = data.aliases.get(&last_name) {
                        if text != *new_val {
                            replacements.push(Replacement {
                                start: child.start_byte(),
                                end: child.end_byte(),
                                text: new_val.clone(),
                            });
                        }
                    }
                    last_name = String::new();
                }
            }
        }
    }

    // We only update the standard keys that were in the original defsrc.
    // The aliases are handled via Replacement in defalias above.
    let num_standard_keys = data.defsrc.len();

    for (i, layer_node) in deflayer_nodes.iter().enumerate() {
        if i < data.layers.len() {
            let target_layer = &data.layers[i];
            let mut inner_cursor = layer_node.walk();
            let mut symbol_count = 0;
            let mut binding_idx = 0;

            for child in layer_node.children(&mut inner_cursor) {
                let kind = child.kind();
                if kind == "symbol" || kind == "boolean" || kind == "number" || kind == "list" {
                    let text = child.utf8_text(source).unwrap_or("");
                    if text == "deflayer" {
                        continue;
                    }
                    if symbol_count == 0 {
                        // Rename layer if needed
                        if text != target_layer.name {
                            replacements.push(Replacement {
                                start: child.start_byte(),
                                end: child.end_byte(),
                                text: target_layer.name.clone(),
                            });
                        }
                        symbol_count += 1;
                    } else {
                        // Recurse to find all actual bindings
                        update_bindings(
                            child,
                            source,
                            &target_layer.bindings,
                            &mut binding_idx,
                            num_standard_keys,
                            &mut replacements,
                        );
                    }
                }
            }
        }
    }

    // Handle deflayermap updates
    // Find deflayermap layers in data
    let deflayermap_layers: Vec<_> = data.layers
        .iter()
        .filter(|l| matches!(l.layer_type, LayerType::Deflayermap))
        .collect();

    for (i, map_node) in deflayermap_nodes.iter().enumerate() {
        if let Some(target_layer) = deflayermap_layers.get(i) {
            let mut inner_cursor = map_node.walk();
            let mut symbol_count = 0;

            for child in map_node.children(&mut inner_cursor) {
                let kind = child.kind();
                if kind == "symbol" || kind == "boolean" || kind == "number" {
                    let text = child.utf8_text(source).unwrap_or("");

                    if text == "deflayermap" {
                        continue;
                    }

                    if symbol_count == 0 {
                        // Layer name
                        if text != target_layer.name {
                            replacements.push(Replacement {
                                start: child.start_byte(),
                                end: child.end_byte(),
                                text: target_layer.name.clone(),
                            });
                        }
                        symbol_count += 1;
                    } else if symbol_count % 2 == 1 {
                        // This is a key name in deflayermap
                        // Find if this key has been updated
                        if let Some(_new_action) = target_layer.key_bindings.get(text) {
                            // Look ahead to find the action node (next sibling)
                            // We need to get the next child in the iteration
                            // Since we can't easily peek, we'll mark this and handle in the next iteration
                        }
                        symbol_count += 1;
                    } else {
                        // This is an action - check if it needs updating
                        let _key_idx = (symbol_count - 1) / 2;
                        // Get the key name from the previous iteration
                        symbol_count += 1;
                    }
                } else if kind == "list" {
                    // This is an action (list form)
                    let _text = child.utf8_text(source).unwrap_or("");
                    symbol_count += 1;
                }
            }
        }
    }

    replacements.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in replacements {
        content.replace_range(r.start..r.end, &r.text);
    }

    Ok(content)
}

/// Insert a key into defsrc at the correct position based on standard layout order
fn insert_key_into_defsrc(
    content: &mut String,
    key_name: &str,
    _is_mac: bool,
    _is_laptop: bool,
) -> Result<(), String> {
    // Parse the content to find defsrc
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_scheme::LANGUAGE.into())
        .map_err(|e| format!("Failed to set language: {}", e))?;
    
    let tree = parser.parse(content.as_bytes(), None)
        .ok_or("Failed to parse content")?;
    
    let source = content.as_bytes();
    let root = tree.root_node();

    // Find defsrc node
    let defsrc_nodes = find_kanata_node(root, source, "defsrc");
    if defsrc_nodes.is_empty() {
        return Err("No defsrc found".to_string());
    }
    
    let defsrc_node = defsrc_nodes[0];

    // Find the last key in defsrc
    let mut last_key_end = defsrc_node.start_byte();
    let mut inner_cursor = defsrc_node.walk();
    for child in defsrc_node.children(&mut inner_cursor) {
        let kind = child.kind();
        if kind == "symbol" || kind == "boolean" || kind == "number" {
            let text = child.utf8_text(source).unwrap_or("");
            if text != "defsrc" {
                last_key_end = child.end_byte();
            }
        }
    }
    
    // Insert the key
    content.insert_str(last_key_end, &format!(" {}", key_name));
    
    Ok(())
}

/// Add a binding slot to all deflayer blocks
fn add_binding_to_all_layers(
    content: &mut String,
    binding: &str,
    _insert_idx: usize, // No longer used, always appends
) -> Result<(), String> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_scheme::LANGUAGE.into())
        .map_err(|e| format!("Failed to set language: {}", e))?;
    
    let tree = parser.parse(content.as_bytes(), None)
        .ok_or("Failed to parse content")?;
    
    let source = content.as_bytes();
    let root = tree.root_node();

    #[allow(dead_code)]
    struct Replacement {
        start: usize,
        end: usize,
        text: String,
    }
    
    let mut replacements = Vec::new();

    let deflayer_nodes = find_kanata_node(root, source, "deflayer");
    
    for layer_node in deflayer_nodes {
        // Find the last binding in the layer
        let mut last_binding_end = layer_node.start_byte();
        let mut inner_cursor = layer_node.walk();
        for child in layer_node.children(&mut inner_cursor) {
            let kind = child.kind();
            if kind == "symbol" || kind == "boolean" || kind == "number" || kind == "list" {
                let text = child.utf8_text(source).unwrap_or("");
                if text != "deflayer" {
                    last_binding_end = child.end_byte();
                }
            }
        }
        
        replacements.push(Replacement {
            start: last_binding_end,
            end: last_binding_end,
            text: format!(" {}", binding),
        });
    }
    
    // Apply replacements in reverse order
    replacements.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in replacements {
        content.insert_str(r.start, &r.text);
    }
    
    Ok(())
}

async fn parse_kanata_api(Json(req): Json<KeymapRequest>) -> impl IntoResponse {
    info!(
        "Received parse kanata request, content length: {}, is_mac: {}, is_laptop: {}",
        req.content.len(),
        req.is_mac,
        req.is_laptop
    );
    match parse_kanata_with_tree_sitter(&req.content, req.is_mac, req.is_laptop) {
        Ok(data) => {
            info!(
                "Successfully parsed kanata with {} keys and {} layers",
                data.physical_layout.len(),
                data.layers.len()
            );
            (StatusCode::OK, Json(data)).into_response()
        }
        Err(e) => {
            error!("Kanata Parse error: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

async fn save_kanata_api(Json(req): Json<SaveKeymapRequest>) -> impl IntoResponse {
    info!("Received save kanata request");
    match generate_kanata_kbd(&req.original_content, &req.data) {
        Ok(content) => {
            // Validate the generated content
            match parse_kanata_with_tree_sitter(&content, false, false) {
                Ok(_) => {
                    info!(
                        "Successfully generated and validated new kanata KBD, length: {}",
                        content.len()
                    );
                    (StatusCode::OK, Json(SaveKeymapResponse { content })).into_response()
                }
                Err(e) => {
                    error!("Generated Kanata failed validation: {}", e);
                    (StatusCode::BAD_REQUEST, format!("Generated file is invalid: {}", e)).into_response()
                }
            }
        }
        Err(e) => {
            error!("Kanata Generation error: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

fn app() -> Router {
    let mut app_wasm_serve = ServeDir::new("app_wasm");
    if option_env!("AXUM_PRECOMPRESSED_WASM").is_some() {
        app_wasm_serve = app_wasm_serve.precompressed_br();
    }
    let app_wasm_serve = get_service(app_wasm_serve).handle_error(handle_error);
    let static_serve = get_service(ServeDir::new("static")).handle_error(handle_error);
    let dist_serve = get_service(ServeDir::new("bundle/dist")).handle_error(handle_error);
    let route_service = RoutableService::<thockflow::Route, _, _>::new(
        get(index),
        route("/api/parse-keymap", post(parse_keymap_api))
            .route("/api/save-keymap", post(save_keymap_api))
            .route("/api/patch-keymap", post(patch_keymap_api))
            .route("/api/parse-kanata", post(parse_kanata_api))
            .route("/api/save-kanata", post(save_kanata_api))
            .route(*APP_JS_PATH, app_wasm_serve.clone())
            .route(*APP_WASM_PATH, app_wasm_serve)
            // Serve built assets from Vite dist first
            .route("/assets/*path", dist_serve)
            // Fallback to legacy static dir
            .fallback(static_serve),
    );
    Router::new()
        .fallback(route_service)
        .layer(Extension(INDEX_HTML.to_string()))
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let app = app();

    if lambda_web::is_running_on_lambda() {
        info!("starting server on lambda");
        lambda_web::run_hyper_on_lambda(app)
            .await
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    } else {
        let addr = std::env::var("HTTP_LISTEN_ADDR").unwrap_or("127.0.0.1:8080".into());
        info!("starting server on {}", addr);
        axum::Server::bind(&addr.parse()?)
            .serve(app.into_make_service())
            .await?;
    }

    Ok(())
}

#[derive(Clone)]
struct RoutableService<R, S: Clone, F: Clone> {
    r: PhantomData<R>,
    s_ready: bool,
    s: S,
    f_ready: bool,
    f: F,
}

impl<R, S: Clone, F: Clone> RoutableService<R, S, F> {
    pub fn new(s: S, f: F) -> Self {
        Self {
            s,
            f,
            s_ready: false,
            f_ready: false,
            r: PhantomData,
        }
    }
}

impl<R, S, F> Service<Request<Body>> for RoutableService<R, S, F>
where
    R: Routable,
    S: Service<Request<Body>, Error = Infallible> + Clone,
    S::Response: IntoResponse,
    S::Future: Send + 'static,
    F: Service<Request<Body>, Error = Infallible> + Clone,
    F::Response: IntoResponse,
    F::Future: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        loop {
            match (self.s_ready, self.f_ready) {
                (true, true) => {
                    return Ok(()).into();
                }
                (false, _) => {
                    ready!(self.s.poll_ready(cx))?;
                    self.s_ready = true;
                }
                (_, false) => {
                    ready!(self.f.poll_ready(cx))?;
                    self.f_ready = true;
                }
            }
        }
    }

    //  send known paths to Yew to be SSR'd, otherwise fall-back to `f`
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        match <R as Routable>::recognize(req.uri().path()).is_some() {
            true => {
                self.s_ready = false;
                let fut = self.s.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.into_response())
                })
            }
            false => {
                self.f_ready = false;
                let fut = self.f.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.into_response())
                })
            }
        }
    }
}

fn route(path: &str, method_router: MethodRouter) -> Router {
    Router::new().route(path, method_router)
}
