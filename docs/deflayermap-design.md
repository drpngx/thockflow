# Design and Implementation Document: Kanata `deflayermap` Support

**Status:** ✅ Implemented  
**Date:** 2026-03-10

## Overview

This document describes the implementation of kanata's `(deflayermap)` feature in ThockFlow. The `deflayermap` construct allows defining a layer by specifying only the changed keys relative to a source layer, rather than redefining all keys.

## Feature Description

### What is `deflayermap`?

From the [kanata documentation](https://github.com/jtroo/kanata/blob/main/docs/config.adoc):

```scheme
(deflayermap (layer-name)
  key1 action1
  key2 action2
  ...)
```

Unlike `(deflayer ...)` which requires a full list of all keys in the same order as `defsrc`, `deflayermap` uses a key-to-action mapping syntax. This is particularly useful for:

1. **Base layer customization**: Making small tweaks to the base layer without redefining all keys
2. **Overlay layers**: Creating layers that only change a few keys
3. **Readability**: Making the configuration more concise and easier to understand

### `process-unmapped-keys` Configuration

The `defcfg` option `process-unmapped-keys` controls how unmapped keys behave:

```scheme
(defcfg
  process-unmapped-keys yes        ; All keys in defsrc are implicitly available
  ;; OR
  process-unmapped-keys (all-except lctl ralt)  ; All keys except these
)
```

**Behavior:**
- When `yes`: All keys defined in `defsrc` are processed even if not explicitly mapped in a layer
- When `no` (default): Only explicitly mapped keys are processed; unmapped keys do nothing
- When `(all-except ...)`: All keys except the specified ones are processed

## Implementation Design

### Data Model Changes

#### 1. Extend `Layer` struct to track origin type

```rust
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub bindings: Vec<String>,
    #[serde(default)]
    pub layer_type: LayerType,  // NEW
    #[serde(default)]
    pub source_layer: Option<String>,  // NEW: for deflayermap, the source layer name (usually defsrc)
    #[serde(default)]
    pub key_bindings: HashMap<String, String>,  // NEW: original key->action mappings for deflayermap
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum LayerType {
    #[default]
    Deflayer,       // Traditional (deflayer ...)
    Deflayermap,    // Mapping-based (deflayermap ...)
}
```

#### 2. Extend `KeymapData` to store process-unmapped-keys configuration

```rust
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct KeymapData {
    pub physical_layout: Vec<PhysicalKey>,
    pub layers: Vec<Layer>,
    pub includes: Vec<String>,
    pub aliases: HashMap<String, String>,
    pub defsrc: Vec<String>,
    pub unmapped_names: Vec<String>,
    // NEW fields for deflayermap support
    #[serde(default)]
    pub process_unmapped_keys: ProcessUnmappedKeys,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum ProcessUnmappedKeys {
    #[default]
    No,
    Yes,
    AllExcept(Vec<String>),  // Keys to exclude
}
```

### Parsing Logic

#### 1. Parse `defcfg` for `process-unmapped-keys`

In `parse_kanata_with_tree_sitter()`:

```rust
// Find defcfg nodes
let defcfg_nodes = find_kanata_node(root, source, "defcfg");
let mut process_unmapped_keys = ProcessUnmappedKeys::No;

for cfg_node in defcfg_nodes {
    // Parse key-value pairs within defcfg
    // Look for "process-unmapped-keys" setting
    // Handle: yes, no, (all-except key1 key2 ...)
}
```

#### 2. Parse `deflayermap` nodes

Add new function to find and parse `deflayermap`:

```rust
fn parse_deflayermap(
    node: tree_sitter::Node,
    source: &[u8],
    defsrc: &[String],           // Source layer key order
    process_unmapped: &ProcessUnmappedKeys,
) -> Option<Layer> {
    // Structure: (deflayermap name key1 action1 key2 action2 ...)
    // Or: (deflayermap (from-layer other) name key1 action1 ...)
    
    // 1. Extract layer name
    // 2. Extract key->action mappings as HashMap
    // 3. Build full bindings vector:
    //    - Start with source layer bindings (or "_" for transparent)
    //    - Apply explicit mappings from deflayermap
    // 4. Set layer_type = LayerType::Deflayermap
    // 5. Store key_bindings for round-trip serialization
}
```

#### 3. Binding Resolution Algorithm

When building layer bindings from a `deflayermap`:

```rust
fn build_deflayermap_bindings(
    defsrc: &[String],
    source_layer_bindings: &[String],  // Usually the base layer
    explicit_mappings: &HashMap<String, String>,
    process_unmapped: &ProcessUnmappedKeys,
) -> Vec<String> {
    let mut bindings = Vec::new();
    
    for (idx, key_name) in defsrc.iter().enumerate() {
        // Check if key has explicit mapping in deflayermap
        if let Some(action) = explicit_mappings.get(key_name) {
            bindings.push(action.clone());
        } else {
            // No explicit mapping - determine default behavior
            match process_unmapped {
                ProcessUnmappedKeys::Yes => {
                    // Key is available with its base action
                    bindings.push(source_layer_bindings.get(idx).cloned().unwrap_or_else(|| "_".to_string()));
                }
                ProcessUnmappedKeys::No => {
                    // Key is not processed (transparent/"_")
                    bindings.push("_".to_string());
                }
                ProcessUnmappedKeys::AllExcept(exclude) => {
                    if exclude.contains(key_name) {
                        bindings.push("_".to_string());
                    } else {
                        bindings.push(source_layer_bindings.get(idx).cloned().unwrap_or_else(|| "_".to_string()));
                    }
                }
            }
        }
    }
    
    bindings
}
```

### Serialization Logic

#### Writing `deflayermap` (Round-trip)

When generating kanata output, preserve the original format:

```rust
fn generate_kanata_kbd(original: &str, data: &KeymapData) -> Result<String> {
    // ... existing code ...
    
    for (i, layer) in data.layers.iter().enumerate() {
        match layer.layer_type {
            LayerType::Deflayer => {
                // Use existing deflayer generation logic
            }
            LayerType::Deflayermap => {
                // Generate (deflayermap name key1 val1 key2 val2 ...)
                // Only write the differences from source layer
                generate_deflayermap(&layer, &data.defsrc, data.layers.get(0))?;
            }
        }
    }
}

fn generate_deflayermap(
    layer: &Layer,
    defsrc: &[String],
    base_layer: Option<&Layer>,
) -> String {
    // Compare current bindings with base layer
    // Only include keys where they differ
    let mut mappings = Vec::new();
    
    for (idx, (current, base)) in layer.bindings.iter().zip(base_layer.map(|l| &l.bindings).iter().flatten()).enumerate() {
        if current != base {
            if let Some(key_name) = defsrc.get(idx) {
                mappings.push(format!("{} {}", key_name, current));
            }
        }
    }
    
    // Also check for keys that were explicitly mapped in original deflayermap
    // but might be the same as base (user explicitly set them)
    for (key, action) in &layer.key_bindings {
        mappings.push(format!("{} {}", key, action));
    }
    
    format!("(deflayermap {}\n  {}\n)", layer.name, mappings.join("\n  "))
}
```

## Test Cases

### Test 1: Parse deflayermap with process-unmapped-keys yes

```scheme
(defcfg
  process-unmapped-keys yes
)

(defsrc
  esc  f1   a    b
)

(deflayer base
  esc  f1   a    b
)

(deflayermap (nav)
  a    (layer-toggle symbols)
  b    left
)
```

**Expected:**
- `nav` layer should have bindings: `["esc", "f1", "(layer-toggle symbols)", "left"]`
- Layer type should be `Deflayermap`
- `key_bindings` should contain `{a: "(layer-toggle symbols)", b: "left"}`

### Test 2: Parse deflayermap with process-unmapped-keys no

```scheme
(defcfg
  process-unmapped-keys no
)

(defsrc
  esc  f1   a    b
)

(deflayer base
  esc  f1   a    b
)

(deflayermap (nav)
  a    (layer-toggle symbols)
  b    left
)
```

**Expected:**
- `nav` layer should have bindings: `["_", "_", "(layer-toggle symbols)", "left"]`
- Unmapped keys are transparent ("_")

### Test 3: Parse deflayermap with all-except

```scheme
(defcfg
  process-unmapped-keys (all-except esc)
)

(defsrc
  esc  f1   a    b
)

(deflayer base
  esc  f1   a    b
)

(deflayermap (nav)
  b    left
)
```

**Expected:**
- `nav` layer should have bindings: `["_", "f1", "a", "left"]`
- `esc` is excluded (transparent), others inherit from base

### Test 4: Round-trip Serialization

1. Parse a file with `deflayermap`
2. Modify a binding in another layer
3. Save the file
4. Verify the `deflayermap` is preserved in its original format

### Test 5: Mixed deflayer and deflayermap

```scheme
(defsrc a b c d)

(deflayer base _ _ _ _)

(deflayer full
  x y z w
)

(deflayermap (partial)
  a x
  b y
)
```

**Expected:**
- `full` is a normal Deflayer with 4 bindings
- `partial` is a Deflayermap with bindings based on process-unmapped-keys setting

## Implementation Plan

### Phase 1: Data Model Updates
1. Add `LayerType` enum
2. Add `layer_type` and `key_bindings` fields to `Layer`
3. Add `ProcessUnmappedKeys` enum
4. Add `process_unmapped_keys` field to `KeymapData`

### Phase 2: Parsing
1. Parse `defcfg` to extract `process-unmapped-keys`
2. Add `find_kanata_node` call for `deflayermap`
3. Implement `parse_deflayermap` function
4. Integrate deflayermap parsing into main parse flow

### Phase 3: Serialization
1. Update `generate_kanata_kbd` to handle deflayermap layers
2. Implement diff logic to write only changed keys
3. Preserve original format on round-trip

### Phase 4: Testing
1. Add unit tests for parsing
2. Add unit tests for serialization
3. Add integration tests for round-trip

## Files to Modify

1. **`src/keymap/mod.rs`** - Add `LayerType` and extend `Layer` struct
2. **`server/src/main.rs`** - Update parsing and serialization functions:
   - `parse_kanata_with_tree_sitter()`
   - `generate_kanata_kbd()`
   - Add `parse_deflayermap()` helper

## Backward Compatibility

- Existing `deflayer` layers should continue to work unchanged
- `LayerType` defaults to `Deflayer` for backward compatibility
- `process_unmapped_keys` defaults to `No` for backward compatibility

## Implementation Summary

### Completed Work

All phases of the implementation have been completed:

1. **Data Model Changes** (`src/keymap/mod.rs`):
   - Added `LayerType` enum with `Deflayer` and `Deflayermap` variants
   - Added `layer_type`, `source_layer`, and `key_bindings` fields to `Layer` struct
   - Added `ProcessUnmappedKeys` enum with `No`, `Yes`, and `AllExcept` variants
   - Added `process_unmapped_keys` field to `KeymapData`

2. **Parsing Implementation** (`server/src/main.rs`):
   - Added `parse_defcfg()` to extract `process-unmapped-keys` from `defcfg`
   - Added `parse_deflayermap()` to parse `(deflayermap ...)` expressions
   - Updated `parse_kanata_with_tree_sitter()` to handle both `deflayer` and `deflayermap`
   - Properly handles tree-sitter-scheme's inclusion of parentheses as child nodes

3. **Serialization** (`server/src/main.rs`):
   - Updated `generate_kanata_kbd()` to handle deflayermap layers
   - Preserves original format on round-trip

4. **Tests** (`server/src/main.rs`):
   - `test_parse_deflayermap_with_process_unmapped_keys_yes` - Tests `process-unmapped-keys yes`
   - `test_parse_deflayermap_with_process_unmapped_keys_no` - Tests `process-unmapped-keys no`
   - `test_parse_deflayermap_with_all_except` - Tests `process-unmapped-keys (all-except ...)`
   - `test_deflayermap_roundtrip` - Tests serialization round-trip
   - `test_mixed_deflayer_and_deflayermap` - Tests mixed layer types

### Key Implementation Details

1. **Layer Name in Parentheses**: The deflayermap syntax requires the layer name to be wrapped in parentheses: `(deflayermap (layer-name) ...)`. The parser extracts the layer name from within this nested list structure.

2. **Tree-sitter-scheme AST**: Tree-sitter-scheme includes the parentheses `(` and `)` as separate child nodes in the AST. The parsing code accounts for this by filtering out parenthesis nodes when iterating through children:

```rust
// Skip parentheses - only keep actual content
if kind != "(" && kind != ")" {
    children.push(child);
}
```

3. **Layer Name Extraction**: The layer name is extracted from a nested list node:

```rust
// Second child should be a list containing the layer name: (layer-name)
let layer_name = if children[1].kind() == "list" {
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
    children[1].utf8_text(source).unwrap_or("").to_string()
};
```

### Test Results

All tests pass:
- `//:thockflow_test` - Library tests
- `//server:server_test` - Server tests including new deflayermap tests
- `//server:keymap_svg` - Keymap SVG binary compiles correctly
