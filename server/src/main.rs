use std::collections::HashMap;
use std::convert::Infallible;
use std::marker::PhantomData;

use anyhow::Result;
use axum::body::{Body, BoxBody};
use axum::extract::Query;
use axum::http::{header, HeaderMap, HeaderValue, Request, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{get_service, MethodRouter, post};
use axum::Extension;
use axum::{routing::get, Router, Json};
use futures::future::BoxFuture;
use futures::ready;
use thockflow::ServerAppProps;
use thockflow::keymap::{KeymapData, PhysicalKey, Layer};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
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

use log::{info, error};

#[derive(Deserialize, Serialize)]
struct KeymapRequest {
    content: String,
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

async fn parse_keymap_api(Json(req): Json<KeymapRequest>) -> impl IntoResponse {
    info!("Received parse request, content length: {}", req.content.len());
    match parse_keymap_with_tree_sitter(&req.content) {
        Ok(data) => {
            info!("Successfully parsed keymap with {} keys and {} layers", data.physical_layout.len(), data.layers.len());
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
            info!("Successfully generated new keymap DTS, length: {}", content.len());
            (StatusCode::OK, Json(SaveKeymapResponse { content })).into_response()
        }
        Err(e) => {
            error!("Generation error: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

fn generate_keymap_dts(original: &str, data: &KeymapData) -> Result<String> {
    let mut content = original.to_string();
    
    // 1. Handle #includes
    let include_re = regex::Regex::new(r#"(?m)^#include\s*[<"](.+?)[>"]"#).unwrap();
    let mut existing_includes: std::collections::HashSet<String> = include_re.captures_iter(original)
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
                let token = token.trim_matches(|c| c == '&' || c == '<' || c == '>' || c == ';' || c == ' ');
                if let Some(behavior) = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(token) || b.name == token) {
                    if !behavior.is_default && !existing_includes.contains(behavior.include_file) {
                        new_includes.push(format!("#include <{}>", behavior.include_file));
                        existing_includes.insert(behavior.include_file.to_string());
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
    let tree = parser.parse(content.as_bytes(), None).ok_or_else(|| anyhow::anyhow!("Failed to parse DTS"))?;
    
    fn find_keymap_node<'a>(node: tree_sitter::Node<'a>, source: &[u8]) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "node" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "property" {
                    let prop_name = child.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                    if prop_name == "compatible" {
                        let prop_value = child.child_by_field_name("value").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
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
    let original_layer_nodes: Vec<tree_sitter::Node> = keymap_node.children(&mut cursor)
        .filter(|n| n.kind() == "node")
        .collect();

    struct Replacement {
        start: usize,
        end: usize,
        text: String,
    }
    let mut replacements = Vec::new();

    // Helper to generate bindings string
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
                let b = target_layer.bindings.get(key_idx).map(|s| s.as_str()).unwrap_or("&none");
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
                    let prop_name = child.child_by_field_name("name").map(|n| n.utf8_text(content.as_bytes()).unwrap_or("")).unwrap_or("");
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
            replacements.push(Replacement { start, end, text: String::new() });
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
    let tree = parser.parse(content, None).ok_or_else(|| anyhow::anyhow!("Failed to parse DTS"))?;
    
    let root_node = tree.root_node();
    // ... existing error checking ...
    if root_node.has_error() {
        // Find where the error is
        let mut error_pos = String::new();
        fn find_error(node: tree_sitter::Node, source: &[u8], pos: &mut String) {
            if node.has_error() {
                if node.kind() == "ERROR" {
                    *pos = format!("Tree-sitter parse error at line {}, column {}", node.start_position().row + 1, node.start_position().column + 1);
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        find_error(child, source, pos);
                        if !pos.is_empty() { return; }
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
    let includes: Vec<String> = include_re.captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect();

    // Recursive traversal to find nodes
    fn traverse(node: tree_sitter::Node, source: &[u8], physical_layout: &mut Vec<PhysicalKey>, layers: &mut Vec<Layer>) {
        if node.kind() == "node" {
            let node_name = node.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
            info!("Visiting node: {}", node_name);
            
            // Check properties for "compatible"
            let mut is_phys = false;
            let mut is_keymap = false;

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "property" {
                    let prop_name = child.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                    if prop_name == "compatible" {
                        let prop_value = child.child_by_field_name("value").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
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
                        let prop_name = child.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                        if prop_name == "keys" {
                            info!("  Parsing keys property...");
                            let mut cursor = child.walk();
                            for val_node in child.children(&mut cursor) {
                                if val_node.kind() != "identifier" {
                                    let raw_val = val_node.utf8_text(source).unwrap_or("");
                                    let num_re = r"\(?([\d-]+)\)?";
                                    // Format: width, height, x, y, rotation, col_offset, row_offset
                                    let key_re_str = format!(r"&key_physical_attrs\s+{}\s+{}\s+{}\s+{}\s+{}\s+{}\s+{}", num_re, num_re, num_re, num_re, num_re, num_re, num_re);
                                    let key_regex = regex::Regex::new(&key_re_str).unwrap();
                                    for cap in key_regex.captures_iter(raw_val) {
                                        physical_layout.push(PhysicalKey {
                                            width: cap[1].parse().unwrap_or(100),
                                            height: cap[2].parse().unwrap_or(100),
                                            x: cap[3].parse().unwrap_or(0),
                                            y: cap[4].parse().unwrap_or(0),
                                            rotation: cap[5].parse().unwrap_or(0),
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
                            if inner_child.kind() == "node_name" || inner_child.kind() == "identifier" {
                                if layer_name.is_empty() {
                                    layer_name = inner_child.utf8_text(source).unwrap_or("").to_string();
                                }
                            }
                            if inner_child.kind() == "property" {
                                let prop_name = inner_child.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                                if prop_name == "bindings" {
                                    let mut prop_cursor = inner_child.walk();
                                    for val_node in inner_child.children(&mut prop_cursor) {
                                        if val_node.kind() != "identifier" {
                                            let raw_val = val_node.utf8_text(source).unwrap_or("");
                                            
                                            // Improved parsing using ZMK_BEHAVIORS
                                            let tokens: Vec<&str> = raw_val.split_whitespace().collect();
                                            let mut i = 0;
                                            while i < tokens.len() {
                                                let token = tokens[i].trim_matches(|c| c == '<' || c == '>' || c == ';' || c == ' ');
                                                if token.starts_with('&') {
                                                    let behavior_name = &token[1..];
                                                    // Find behavior
                                                    let behavior = ZMK_BEHAVIORS.iter().find(|b| b.label == Some(behavior_name) || b.name == behavior_name);
                                                    let mut binding = token.to_string();
                                                    if let Some(b) = behavior {
                                                        let cells = b.binding_cells;
                                                        for _ in 0..cells {
                                                            i += 1;
                                                            if i < tokens.len() {
                                                                binding.push(' ');
                                                                binding.push_str(tokens[i].trim_matches(|c| c == '>' || c == ';'));
                                                            }
                                                        }
                                                    }
                                                    bindings.push(binding);
                                                }
                                                i += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if !bindings.is_empty() {
                            info!("  Found layer: {} with {} bindings", layer_name, bindings.len());
                            layers.push(Layer { name: layer_name, bindings });
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

    traverse(root_node, content.as_bytes(), &mut physical_layout, &mut layers);

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
        info!("Physical layout missing, attempting to match by key count: {}", key_count);
        
        // Find layouts with matching key count
        let matches: Vec<_> = ZMK_LAYOUTS.iter()
            .filter(|l| l.keys.len() == key_count)
            .collect();
        
        if !matches.is_empty() {
            // Heuristic: prioritize layouts with "default" or "6col"
            let matched_layout = matches.iter()
                .find(|l| l.name.contains("default") || l.display_name.map_or(false, |dn| dn.to_lowercase().contains("default")))
                .or_else(|| matches.iter().find(|l| l.name.contains("6col")))
                .unwrap_or(&matches[0]);
            
            info!("Matched layout: {} from {}", matched_layout.name, matched_layout.source_file);
            physical_layout = matched_layout.keys.iter().map(|k| PhysicalKey {
                width: k.width,
                height: k.height,
                x: k.x,
                y: k.y,
                rotation: k.rotation,
            }).collect();
        }
    }

    if physical_layout.is_empty() {
        return Err(anyhow::anyhow!("Missing physical layout (zmk,physical-layout compatible node) and no match found in database for {} keys", layers.get(0).map_or(0, |l| l.bindings.len())));
    }
    if layers.is_empty() {
        return Err(anyhow::anyhow!("Missing keymap layers (zmk,keymap compatible node)"));
    }

    Ok(KeymapData { physical_layout, layers, includes })
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
        assert!(result.physical_layout.len() >= 52, "Expected at least 52 layout keys, got {}", result.physical_layout.len());
        assert!(result.layers.len() > 0, "Expected at least one layer");
        
        // Check some bindings to see if they were grouped correctly
        let first_layer = &result.layers[0];
        assert!(first_layer.bindings.contains(&"&kp GRAVE".to_string()));
        
        // Check 2-argument binding (bt)
        let adj_layer = &result.layers[5]; // Assuming adj_layer is at index 5
        assert!(adj_layer.name == "adj_layer");
        assert!(adj_layer.bindings.contains(&"&bt BT_SEL 0".to_string()));
        
        println!("Successfully parsed {} keys and {} layers", result.physical_layout.len(), result.layers.len());
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
                    .body(Body::from(serde_json::to_string(&KeymapRequest {
                        content: content.to_string(),
                    }).unwrap()))
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
        use thockflow::keymap::behaviors::{ZMK_BEHAVIORS, ParameterType};
        
        let lt = ZMK_BEHAVIORS.iter().find(|b| b.label == Some("lt")).expect("lt behavior missing");
        assert_eq!(lt.binding_cells, 2);
        assert_eq!(lt.parameter_metadata.len(), 2);
        assert_eq!(lt.parameter_metadata[0], ParameterType::Layer);
        assert_eq!(lt.parameter_metadata[1], ParameterType::Keycode);

        let mt = ZMK_BEHAVIORS.iter().find(|b| b.label == Some("mt")).expect("mt behavior missing");
        assert_eq!(mt.binding_cells, 2);
        assert_eq!(mt.parameter_metadata.len(), 2);
        assert_eq!(mt.parameter_metadata[0], ParameterType::Modifier);
        assert_eq!(mt.parameter_metadata[1], ParameterType::Keycode);

        let bt = ZMK_BEHAVIORS.iter().find(|b| b.label == Some("bt")).expect("bt behavior missing");
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
        
        assert!(keycodes::is_regular_key("AC_BACK"));
        assert!(!keycodes::is_modifier("AC_BACK"));
        
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
                PhysicalKey { x: 0, y: 0, width: 100, height: 100, rotation: 0 },
                PhysicalKey { x: 200, y: 0, width: 100, height: 100, rotation: 0 },
            ],
            layers: vec![
                Layer { name: "new_layer_0".to_string(), bindings: vec!["&kp X".to_string(), "&kp Y".to_string()] },
            ],
            includes: vec![],
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
                PhysicalKey { x: 0, y: 0, width: 100, height: 100, rotation: 0 },
                PhysicalKey { x: 200, y: 0, width: 100, height: 100, rotation: 0 },
            ],
            layers: vec![
                Layer { name: "layer_0".to_string(), bindings: vec!["&kp LONG_BINDING".to_string(), "&kp B".to_string()] },
                Layer { name: "layer_1".to_string(), bindings: vec!["&kp A".to_string(), "&kp SHORT".to_string()] },
            ],
            includes: vec![],
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
                PhysicalKey { x: 0, y: 0, width: 100, height: 100, rotation: 0 },
                PhysicalKey { x: 200, y: 0, width: 100, height: 100, rotation: 0 },
            ],
            layers: vec![
                Layer { name: "layer_0".to_string(), bindings: vec!["&kp A".to_string(), "&kp B".to_string()] },
                Layer { name: "new_layer".to_string(), bindings: vec!["&kp C".to_string(), "&kp D".to_string()] },
            ],
            includes: vec![],
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
    fn test_parse_uneven_layers() {
        let content = r#"
/ {
    keymap {
        compatible = "zmk,keymap";
        layer_0 {
            bindings = <&kp A &kp B>;
        };
        layer_1 {
            bindings = <&kp C>;
        };
    };
};
"#;
        let result = parse_keymap_with_tree_sitter(content);
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("All layers must have the same number of keys"));
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
                PhysicalKey { x: 0, y: 0, width: 100, height: 100, rotation: 0 },
                PhysicalKey { x: 200, y: 0, width: 100, height: 100, rotation: 0 },
            ],
            layers: vec![
                Layer { name: "layer_0".to_string(), bindings: vec!["&mmv 0".to_string(), "&kp B".to_string()] },
            ],
            includes: vec!["custom.h".to_string()],
        };

        let result = generate_keymap_dts(content, &data).unwrap();
        println!("Result:\n{}", result);
        
        // Should have added both custom.h and mouse_move.dtsi
        assert!(result.contains("#include <custom.h>"));
        assert!(result.contains("#include <behaviors/mouse_move.dtsi>"));
        
        // Check placement: should be after keys.h
        let pos_keys = result.find("#include <dt-bindings/zmk/keys.h>").unwrap();
        let pos_custom = result.find("#include <custom.h>").unwrap();
        let pos_mmv = result.find("#include <behaviors/mouse_move.dtsi>").unwrap();
        
        assert!(pos_custom > pos_keys, "custom.h should be after keys.h");
        assert!(pos_mmv > pos_keys, "mouse_move.dtsi should be after keys.h");
    }

    #[test]
    fn test_svg_generation_hshs52() {
        use thockflow::keymap::generate_svg;
        let content = include_str!("../../static/hshs52.keymap");
        let data = parse_keymap_with_tree_sitter(content).expect("Should parse hshs52");
        
        let svg = generate_svg(&data);
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
        let layer_titles: Vec<_> = root.descendants()
            .filter(|n| n.attribute("class") == Some("layer-title"))
            .collect();
        assert_eq!(layer_titles.len(), data.layers.len(), "Should have correct number of layer titles");
        
        // Check for keys (rect with class "key")
        let keys: Vec<_> = root.descendants()
            .filter(|n| n.attribute("class") == Some("key"))
            .collect();
        assert_eq!(keys.len(), data.layers.len() * data.physical_layout.len(), "Should have correct total number of keys across all layers");
        
        println!("SVG validation passed: {} layers, {} total keys", layer_titles.len(), keys.len());
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
        .replace("</head>", &format!("{}</head>", html_wasm_init_head(init_quote_index)));
    (
        HeaderMap::from_iter([(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))]),
        Html(html),
    )
}

async fn handle_error(e: impl std::fmt::Debug) -> impl IntoResponse {
    eprintln!("{e:?}");
    StatusCode::BAD_REQUEST
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
