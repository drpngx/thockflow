# Keymap Layout Improvements - Design & Implementation

## Overview

This document describes the improvements to the Keymap tab for handling keyboard layouts when no geometry is found in the keymap file, ZMK defaults, or the nickcoutsos contrib repository.

## Goals

1. **Import keyboards from nickcoutsos/keymap-editor-contrib** as an additional layout source
2. **Provide layout selection/generation fallback** when no geometry is available:
   - Match by key count (±3 keys tolerance)
   - Auto-generate "square" layout based on QWERTY/Dvorak/Colemak detection
3. **Ensure save works** by downloading file when File System Access API is unavailable
4. **Comprehensive tests** for the layout matching and generation logic

## Architecture

### Current State

```
┌─────────────────────────────────────────────────────────────┐
│  parse_keymap_with_tree_sitter() in server/src/main.rs     │
│                                                             │
│  1. Parse keymap file → extract physical layout from DTS   │
│  2. If no physical layout:                                 │
│     - Match by exact key count in ZMK_LAYOUTS (static)     │
│  3. Error if no match found                                │
└─────────────────────────────────────────────────────────────┘
```

### Proposed State

```
┌─────────────────────────────────────────────────────────────┐
│  parse_keymap_with_tree_sitter() - Enhanced                │
│                                                             │
│  1. Parse keymap file → extract physical layout            │
│  2. If no physical layout:                                 │
│     a. Try exact match in ZMK_LAYOUTS (existing)           │
│     b. Try match in CONTRIB_LAYOUTS (nickcoutsos)          │
│     c. Return needs_layout_selection with candidates       │
│                                                             │
│  3. New API: /api/layout-candidates/{key_count}            │
│     - Returns layouts with key_count ± 3 tolerance         │
│                                                             │
│  4. New API: /api/generate-square-layout                   │
│     - Takes sample bindings, detects layout type           │
│     - Generates reasonable grid layout                     │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Import Nickcoutsos Contrib Layouts

**File: `server/src/bin/fetch_contrib_layouts.rs`**

New binary to fetch and convert keyboard data from the contrib repository.

```rust
// Fetches keyboard JSONs from GitHub API
// Converts from nickcoutsos format to our ZmkLayout format
// Output: src/keymap/contrib_layouts.rs
```

**Nickcoutsos JSON format:**
```json
{
  "id": "a_dux",
  "name": "A. Dux",
  "layouts": {
    "default_transform": {
      "layout": [
        { "row": 0, "col": 0, "r": -15, "x": 0.11, "y": 1.72, "rx": 0.61, "ry": 2.22, "w": 1.0, "h": 1.0 }
      ]
    }
  }
}
```

**Conversion rules:**
- `x`, `y` in key units → multiply by 100 for our coordinate system
- `r` (rotation degrees) → multiply by 100 for our rotation units
- `rx`, `ry` → multiply by 100
- `w`, `h` (width/height) → multiply by 100, default to 100

**File: `src/keymap/contrib_layouts.rs`**

Generated file similar to `layouts.rs`:
```rust
pub const CONTRIB_LAYOUTS: &[ZmkLayout] = &[
    ZmkLayout {
        name: "a_dux",
        display_name: Some("A. Dux"),
        keys: &[ /* converted keys */ ],
        source_file: "nickcoutsos/keymap-editor-contrib",
    },
    // ... more layouts
];
```

### Phase 2: Layout Selection & Generation API

**New API Endpoint: `GET /api/layout-candidates/{key_count}`**

Returns layouts matching the key count within ±3 tolerance.

```rust
#[derive(Serialize)]
struct LayoutCandidate {
    id: String,
    name: String,
    source: String,  // "zmk" or "contrib"
    key_count: usize,
    preview_svg: Option<String>, // Mini SVG for preview
}

async fn layout_candidates(Path(key_count): Path<usize>) -> impl IntoResponse {
    // Search ZMK_LAYOUTS and CONTRIB_LAYOUTS
    // Return candidates where |layout.keys.len() - key_count| <= 3
}
```

**New API Endpoint: `POST /api/generate-square-layout`**

Generates a square/rectangular layout based on detected layout type.

```rust
#[derive(Deserialize)]
struct GenerateLayoutRequest {
    bindings: Vec<String>,  // Sample bindings from first layer
    target_key_count: usize,
}

#[derive(Serialize)]
struct GeneratedLayout {
    keys: Vec<PhysicalKey>,
    detected_layout: LayoutType,  // Qwerty, Dvorak, Colemak, Unknown
    confidence: f32,
}

async fn generate_square_layout(
    Json(req): Json<GenerateLayoutRequest>
) -> impl IntoResponse {
    // Detect layout type from bindings
    // Generate grid layout with appropriate row/column distribution
}
```

### Phase 3: Layout Detection Heuristics

**Layout Type Detection:**

```rust
enum LayoutType {
    Qwerty,
    Dvorak,
    Colemak,
    Workman,
    Unknown,
}

fn detect_layout_type(bindings: &[String]) -> (LayoutType, f32) {
    // Extract keypresses from bindings (&kp XXX)
    // Check positions of known keys for each layout type
    // Return highest confidence match
}
```

**Detection criteria:**
- QWERTY: Q row starts with Q, W, E, R, T, Y
- Dvorak: Q row starts with ',', ',', ., P, Y, F
- Colemak: Q row starts with Q, W, F, P, G, J

**Square Layout Generation:**

```rust
fn generate_square_layout(key_count: usize, layout_type: LayoutType) -> Vec<PhysicalKey> {
    // Determine reasonable row distribution
    // Common layouts: 60% (58-61 keys), 65% (68 keys), 75% (84 keys), TKL (87 keys)
    let (rows, cols) = match key_count {
        40..=48 => (3, key_count / 3 + 1),   // Small ergo
        49..=60 => (4, key_count / 4 + 1),   // 60% or similar
        61..=75 => (5, key_count / 5 + 1),   // 65%/75%
        76..=90 => (6, key_count / 6 + 1),   // TKL
        _ => {
            let cols = (key_count as f32).sqrt().ceil() as usize;
            let rows = (key_count + cols - 1) / cols;
            (rows, cols)
        }
    };
    
    // Generate keys in grid with standard spacing (100 units)
    // Apply modifier sizes (tab=1.5u, caps=1.75u, shifts=2.25u/2.75u) 
    //   based on detected layout type and position
}
```

### Phase 4: Frontend Updates

**New Component: `LayoutSelector`**

Shown when `parse_keymap_with_tree_sitter` returns `needs_layout_selection: true`.

```rust
#[derive(Deserialize)]
struct ParseKeymapResponse {
    // ... existing fields ...
    needs_layout_selection: bool,
    key_count: usize,
    candidate_layouts: Vec<LayoutCandidate>,
}

// Component shows:
// 1. "Auto-generate square layout" button
//    - Calls /api/generate-square-layout
//    - Shows preview of generated layout
// 2. "Select similar layout" section
//    - Shows candidates within ±3 keys
//    - Preview each with mini-SVG
// 3. "Search all layouts" option
//    - Full searchable list of all available layouts
```

**Save Button Fallback:**

```rust
// In KeymapHome component
let on_save = Callback::from(move |_| {
    // ... existing save logic ...
    
    // If file_handle is None, always download
    if file_handle.is_none() {
        // Download as file
        let blob = web_sys::Blob::new_with_str_sequence(...);
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        // Create anchor, click, revoke URL
    }
});
```

### Phase 5: Testing

**Unit Tests: `src/keymap/layout_generation.rs`**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_detect_qwerty() {
        let bindings = vec!["&kp Q", "&kp W", "&kp E", "&kp R", "&kp T", "&kp Y"];
        let (layout, confidence) = detect_layout_type(&bindings);
        assert!(matches!(layout, LayoutType::Qwerty));
        assert!(confidence > 0.8);
    }
    
    #[test]
    fn test_detect_dvorak() {
        let bindings = vec!["&kp SQT", "&kp COMMA", "&kp DOT", "&kp P", "&kp Y", "&kp F"];
        let (layout, confidence) = detect_layout_type(&bindings);
        assert!(matches!(layout, LayoutType::Dvorak));
    }
    
    #[test]
    fn test_generate_square_60_percent() {
        let keys = generate_square_layout(60, LayoutType::Qwerty);
        assert_eq!(keys.len(), 60);
        // Verify first row has standard QWERTY positions
        assert_eq!(keys[0].x, 0);
        assert_eq!(keys[1].x, 100);
    }
    
    #[test]
    fn test_layout_candidate_matching() {
        // Test ±3 tolerance
        let candidates = find_candidates(60);  // Looking for 60-key layouts
        // Should include layouts with 57-63 keys
        assert!(candidates.iter().any(|c| c.key_count == 57));
        assert!(candidates.iter().any(|c| c.key_count == 63));
        assert!(!candidates.iter().any(|c| c.key_count == 55));  // Too far
    }
}
```

**Integration Tests: `server/src/main.rs`**

```rust
#[tokio::test]
async fn test_layout_candidates_endpoint() {
    let app = app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/layout-candidates/60")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let candidates: Vec<LayoutCandidate> = serde_json::from_slice(&body).unwrap();
    assert!(!candidates.is_empty());
}

#[tokio::test]
async fn test_generate_layout_endpoint() {
    let app = app();
    let req = GenerateLayoutRequest {
        bindings: vec!["&kp Q", "&kp W", "&kp E", "&kp R", "&kp T", "&kp Y"],
        target_key_count: 60,
    };
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/generate-square-layout")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}
```

## Data Structures

### New Types

```rust
// src/keymap/mod.rs (additions)

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum DetectedKeyboardLayout {
    Qwerty,
    Dvorak,
    Colemak,
    Workman,
    Unknown,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GeneratedLayoutInfo {
    pub layout_type: DetectedKeyboardLayout,
    pub confidence: f32,
    pub rows: usize,
    pub cols: usize,
}

// Updated KeymapData to include generation info
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct KeymapData {
    // ... existing fields ...
    
    /// If layout was auto-generated (not from keymap file)
    #[serde(default)]
    pub generated_layout_info: Option<GeneratedLayoutInfo>,
}
```

### API Types

```rust
// server/src/main.rs

#[derive(Serialize)]
struct LayoutCandidate {
    id: String,
    name: String,
    source: String,
    key_count: usize,
    /// Base64-encoded mini SVG preview
    preview_svg: Option<String>,
}

#[derive(Deserialize)]
struct GenerateLayoutRequest {
    bindings: Vec<String>,
    target_key_count: usize,
}

#[derive(Serialize)]
struct GenerateLayoutResponse {
    keys: Vec<PhysicalKey>,
    info: GeneratedLayoutInfo,
}

#[derive(Serialize)]
struct ParseKeymapResponse {
    #[serde(flatten)]
    data: KeymapData,
    needs_layout_selection: bool,
    candidates: Vec<LayoutCandidate>,
}
```

## File Structure

```
src/
├── keymap/
│   ├── mod.rs              # Existing - add GeneratedLayoutInfo
│   ├── layouts.rs          # Existing - ZMK layouts
│   ├── contrib_layouts.rs  # NEW - Nickcoutsos layouts
│   ├── layout_generation.rs # NEW - Square layout generation
│   └── layout_detector.rs  # NEW - QWERTY/Dvorak/Colemak detection
├── lib.rs                  # Re-export new modules

server/
├── src/
│   ├── bin/
│   │   ├── zmk_layouts.rs           # Existing
│   │   └── fetch_contrib_layouts.rs # NEW - Fetch from nickcoutsos
│   └── main.rs                      # Add new API endpoints

static/
├── test_keymaps/
│   ├── no_layout_60.keymap     # NEW - Test keymap without physical layout
│   ├── no_layout_dvorak.keymap # NEW - Dvorak layout test
│   └── no_layout_colemak.keymap # NEW - Colemak layout test
```

## Implementation Order

1. **Layout Detection Module** (`layout_detector.rs`)
   - Implement detect_layout_type()
   - Add unit tests for QWERTY/Dvorak/Colemak detection

2. **Layout Generation Module** (`layout_generation.rs`)
   - Implement generate_square_layout()
   - Add unit tests for various key counts

3. **Contrib Layouts Fetcher** (`fetch_contrib_layouts.rs`)
   - Implement JSON fetching from GitHub
   - Implement format conversion
   - Add to build process

4. **API Endpoints** (update `main.rs`)
   - Add /api/layout-candidates/{key_count}
   - Add /api/generate-square-layout
   - Update parse-keymap to return needs_layout_selection

5. **Frontend Updates** (`src/keymap/mod.rs`)
   - Add LayoutSelector component
   - Update save button to always allow download

6. **Integration Tests**
   - Test full flow with sample keymaps

## Edge Cases

1. **No bindings provided** - Generate generic rectangular grid
2. **Mixed layout signals** - Choose highest confidence, default to QWERTY
3. **Unusual key counts** - Use sqrt-based row/col calculation
4. **Rotated layouts** - Include rotation in generated layout for thumbs
5. **Contrib fetch failure** - Build should succeed with cached data or empty

## Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
# Existing...

[build-dependencies]
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1", features = ["rt-multi-thread"] }
```

## Notes

- The nickcoutsos repository has ~140 keyboards, so caching is important
- Consider adding a `just regenerate-layouts` command to update both ZMK and contrib layouts
- Generated layouts should be deterministic for testing
- Preview SVGs can be generated on-demand rather than pre-computed
