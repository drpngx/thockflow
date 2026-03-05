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
    // Check which ones are missing and add them at the top
    let include_re = regex::Regex::new(r#"(?m)^#include\s*[<"](.+?)[>"]"#).unwrap();
    let existing_includes: std::collections::HashSet<String> = include_re.captures_iter(original)
        .map(|cap| cap[1].to_string())
        .collect();
    
    let mut new_includes = Vec::new();
    for inc in &data.includes {
        if !existing_includes.contains(inc) {
            new_includes.push(format!("#include <{}>", inc));
        }
    }
    
    if !new_includes.is_empty() {
        content.insert_str(0, &(new_includes.join("\n") + "\n\n"));
    }

    // 2. Replace bindings in layers
    // We use tree-sitter to find the exact byte ranges to replace
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_devicetree::LANGUAGE.into())?;
    let tree = parser.parse(&content, None).ok_or_else(|| anyhow::anyhow!("Failed to parse DTS"))?;
    
    #[derive(Debug)]
    struct LayerReplacement {
        start: usize,
        end: usize,
        new_bindings: String,
    }
    
    let mut replacements = Vec::new();
    let mut layers_found = 0;
    
    fn find_layers(node: tree_sitter::Node, source: &[u8], data: &KeymapData, layers_found: &mut usize, replacements: &mut Vec<LayerReplacement>) {
        if node.kind() == "node" {
            let mut is_keymap = false;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "property" {
                    let prop_name = child.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                    if prop_name == "compatible" {
                        let prop_value = child.child_by_field_name("value").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                        if prop_value.contains("zmk,keymap") {
                            is_keymap = true;
                        }
                    }
                }
            }
            
            if is_keymap {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "node" {
                        // This is a layer node
                        if let Some(target_layer) = data.layers.get(*layers_found) {
                            // Find the bindings property
                            let mut inner_cursor = child.walk();
                            for inner_child in child.children(&mut inner_cursor) {
                                if inner_child.kind() == "property" {
                                    let prop_name = inner_child.child_by_field_name("name").map(|n| n.utf8_text(source).unwrap_or("")).unwrap_or("");
                                    if prop_name == "bindings" {
                                        // Found bindings property
                                        if let Some(value_node) = inner_child.child_by_field_name("value") {
                                            // The value node includes the < and >
                                            let start = value_node.start_byte();
                                            let end = value_node.end_byte();
                                            
                                            let mut new_bindings = String::from("<");
                                            for (i, b) in target_layer.bindings.iter().enumerate() {
                                                if i > 0 {
                                                    if i % 10 == 0 {
                                                        new_bindings.push_str("\n                ");
                                                    } else {
                                                        new_bindings.push(' ');
                                                    }
                                                }
                                                new_bindings.push_str(b);
                                            }
                                            new_bindings.push('>');
                                            
                                            replacements.push(LayerReplacement { start, end, new_bindings });
                                        }
                                    }
                                }
                            }
                        }
                        *layers_found += 1;
                    }
                }
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            find_layers(child, source, data, layers_found, replacements);
        }
    }
    
    find_layers(tree.root_node(), content.as_bytes(), data, &mut layers_found, &mut replacements);
    
    // Apply replacements in reverse order to keep indices valid
    replacements.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in replacements {
        content.replace_range(r.start..r.end, &r.new_bindings);
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
    fn test_parse_missing_physical_fallback() {
        // 50 keys matches Kyria 5-col
        let mut bindings = Vec::new();
        for _ in 0..50 {
            bindings.push("&kp A");
        }
        let content = format!(r#"
#include <behaviors.dtsi>
/ {{
    keymap {{
        compatible = "zmk,keymap";
        default_layer {{
            bindings = <{}>;
        }};
    }};
}};"#, bindings.join(" "));

        let result = parse_keymap_with_tree_sitter(&content).expect("Should fall back to DB layout");
        assert_eq!(result.physical_layout.len(), 50);
        assert_eq!(result.layers.len(), 1);
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
