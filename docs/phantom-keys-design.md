# Design and Implementation Document: Kanata Phantom Keys

**Status:** 📝 Design Phase  
**Date:** 2026-03-11

## Overview

This document describes the implementation of "Phantom Keys" in the Kanata tab. Phantom keys are keys that exist in the physical keyboard layout (standard 108-key, Mac, or laptop layouts) but are not defined in the user's `defsrc`. They appear as outlined keys without fill, are accessible via the `j` menu, and can be added to `defsrc` when modified (unless `process-unmapped-keys=yes`).

## Motivation

Currently, the Kanata editor only shows keys that are explicitly defined in `defsrc`. Users who want to add new keys to their configuration must manually edit the `defsrc` section. Phantom keys provide a visual way to:

1. **Discover available keys**: See all keys available on the physical layout
2. **Add keys interactively**: Click a phantom key and assign a binding to automatically add it to `defsrc`
3. **Visual completeness**: See the full keyboard layout for reference

## Feature Description

### What are Phantom Keys?

Phantom keys are physical keys that:
- Exist in the standard layout (STANDARD_108_LAYOUT, MAC_LAYOUT, WIN_LAPTOP_LAYOUT, or MACBOOK_LAYOUT)
- Are NOT in the user's `defsrc` definition
- Are rendered as outlined (not filled) boxes
- Can be clicked/selected via the `j` menu
- When modified, get added to `defsrc` (unless `process-unmapped-keys=yes`)

### Visual Appearance

```
┌─────────────────────────────────────────────────────────────┐
│ Standard Key (in defsrc)      Phantom Key (not in defsrc)   │
│ ┌─────────┐                   ┌─────────┐                   │
│ │         │                   │ ╔═════╗ │  ← Outline only   │
│ │  BIND   │                   │ ║     ║ │                   │
│ │         │                   │ ╚═════╝ │                   │
│ └─────────┘                   └─────────┘                   │
│ White fill, dark border       Transparent, dashed border    │
└─────────────────────────────────────────────────────────────┘
```

### Behavior

| Scenario | Action |
|----------|--------|
| Phantom key clicked | Opens binding popup with empty/default binding |
| Binding saved | Key added to `defsrc` at correct position, becomes a regular key |
| `process-unmapped-keys=yes` | Modified phantom keys do NOT get added to `defsrc` (already works via unmapped) |
| `j` menu | Phantom keys are included in jump mode hints |

## Implementation Design

### 1. Data Model Changes

#### Extend `KeymapData` to track phantom keys

```rust
// In src/keymap/mod.rs

/// Represents a key that exists in the physical layout but not in defsrc
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PhantomKey {
    pub name: String,           // Key name (e.g., "f13", "calc")
    pub position: (i32, i32),   // Physical position (x, y) in layout units
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct KeymapData {
    pub physical_layout: Vec<PhysicalKey>,
    pub layers: Vec<Layer>,
    pub includes: Vec<String>,
    pub aliases: HashMap<String, String>,
    pub defsrc: Vec<String>,           // Keys explicitly in defsrc
    pub unmapped_names: Vec<String>,   // Non-standard keys in defsrc
    pub process_unmapped_keys: ProcessUnmappedKeys,
    pub defvars: Vec<Defvar>,
    // NEW: Phantom keys for this configuration
    #[serde(default)]
    pub phantom_keys: Vec<PhantomKey>,
}
```

#### Track key origins in the combined layout

The `physical_layout` vector contains all keys in display order. We need to track which are phantom:

```rust
/// Type of key in the combined layout
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum KeyOrigin {
    Standard,      // In defsrc, standard key
    Unmapped,      // In defsrc, non-standard key
    Alias,         // Alias entry (not a physical key)
    Phantom,       // NOT in defsrc, available in physical layout
}

// Or alternatively, add metadata to PhysicalKey:
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PhysicalKey {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub rotation: i32,
    pub rx: i32,
    pub ry: i32,
    #[serde(default)]
    pub origin: KeyOrigin,      // NEW
    #[serde(default)]
    pub name: String,           // NEW: key name for phantom/unmapped
}
```

### 2. Server-Side Changes

#### In `parse_kanata_with_tree_sitter()` (server/src/main.rs)

After parsing `defsrc`, compute phantom keys:

```rust
fn compute_phantom_keys(
    defsrc: &[String],
    is_mac: bool,
    is_laptop: bool,
) -> Vec<PhantomKey> {
    let layout = match (is_mac, is_laptop) {
        (true, true) => &*MACBOOK_LAYOUT,
        (true, false) => &*MAC_LAYOUT,
        (false, true) => &*WIN_LAPTOP_LAYOUT,
        (false, false) => &*STANDARD_108_LAYOUT,
    };

    let defsrc_set: HashSet<_> = defsrc.iter().map(|s| s.to_lowercase()).collect();
    let mut phantom_keys = Vec::new();

    for (name, &(x, y)) in layout.iter() {
        // Skip aliases (keys that map to same position)
        let is_alias = layout.iter().any(|(other_name, &(ox, oy))| {
            name != other_name && ox == x && oy == y
        });
        
        if !is_alias && !defsrc_set.contains(name.to_lowercase().as_str()) {
            phantom_keys.push(PhantomKey {
                name: name.to_string(),
                position: (x, y),
            });
        }
    }

    phantom_keys
}
```

Then update the physical layout computation to include phantom keys at their proper positions:

```rust
// In parse_kanata_with_tree_sitter():
let phantom_keys = compute_phantom_keys(&key_names, is_mac, is_laptop);

// Physical layout now includes:
// 1. Standard keys (from defsrc, at their layout positions)
// 2. Unmapped keys (from defsrc, at bottom)
// 3. Phantom keys (NOT in defsrc, at their layout positions)
// 4. Aliases (at bottom)

let physical_layout = compute_standard_kanata_layout_with_phantoms(
    &key_names, 
    &unmapped_names, 
    &phantom_keys,
    &sorted_alias_names, 
    is_mac, 
    is_laptop
);
```

#### New layout computation function

```rust
pub fn compute_standard_kanata_layout_with_phantoms(
    key_names: &[String],
    unmapped_names: &[String],
    phantom_keys: &[PhantomKey],
    alias_names: &[String],
    is_mac: bool,
    is_laptop: bool,
) -> Vec<PhysicalKey> {
    let mut layout = Vec::new();
    let key_width = 1000;
    let key_height = 1000;

    // 1. Process standard keys from defsrc
    for name in key_names {
        if let Some(&(x, y)) = physical_layout.get(name.to_lowercase().as_str()) {
            layout.push(PhysicalKey {
                x, y, width: key_width, height: key_height,
                rotation: 0, rx: 0, ry: 0,
                origin: KeyOrigin::Standard,
                name: name.clone(),
            });
        }
    }

    // 2. Process phantom keys at their proper positions
    for phantom in phantom_keys {
        layout.push(PhysicalKey {
            x: phantom.position.0,
            y: phantom.position.1,
            width: key_width,
            height: key_height,
            rotation: 0, rx: 0, ry: 0,
            origin: KeyOrigin::Phantom,
            name: phantom.name.clone(),
        });
    }

    // 3. Process unmapped keys (at bottom)
    let unmapped_y_start = 6500;
    // ... existing unmapped handling

    // 4. Process aliases (at bottom)
    let alias_y_start = 8000;
    // ... existing alias handling

    // Sort by Y then X to ensure proper tab order
    layout.sort_by(|a, b| {
        (a.y / 10).cmp(&(b.y / 10)).then(a.x.cmp(&b.x))
    });

    layout
}
```

#### Update `generate_kanata_kbd()` for phantom key insertion

When a phantom key is modified, insert it into `defsrc` at the correct position:

```rust
fn insert_key_into_defsrc(
    content: &mut String,
    key_name: &str,
    key_position: (i32, i32),
    is_mac: bool,
    is_laptop: bool,
) -> Result<(), String> {
    // Find where in defsrc this key should be inserted based on standard layout order
    let layout = match (is_mac, is_laptop) {
        (true, true) => &*MACBOOK_LAYOUT,
        (true, false) => &*MAC_LAYOUT,
        (false, true) => &*WIN_LAPTOP_LAYOUT,
        (false, false) => &*STANDARD_108_LAYOUT,
    };

    // Get the standard key order from layout
    let standard_order: Vec<_> = layout.keys().collect();
    
    // Find where this key should go in the standard order
    let target_pos = standard_order.iter().position(|&k| k == key_name);
    
    // Parse defsrc and find insertion point
    // ... tree-sitter logic to find insertion position
    
    // Insert the key at the correct position
    // ... modify content string
    
    Ok(())
}
```

### 3. Frontend Changes

#### In `KanataRenderer` (src/kanata/mod.rs)

Update rendering to distinguish phantom keys:

```rust
// In the key rendering loop:
let is_phantom = pk.origin == KeyOrigin::Phantom;

html! {
    <div 
        onclick={onclick} 
        class={classes!(
            "absolute", "flex", "flex-col", "items-center", "justify-center", 
            "rounded", "cursor-pointer", "transition-all", "select-none",
            if is_phantom {
                // Phantom key styling: outline only
                vec!["border-2", "border-dashed", "border-gray-400", "dark:border-gray-600", "bg-transparent"]
            } else if is_alias_section {
                vec!["bg-blue-50/30", "dark:bg-blue-900/10", "border-blue-200", "dark:border-blue-800"]
            } else if is_unmapped_section {
                vec!["bg-orange-50/30", "dark:bg-orange-900/10", "border-orange-200", "dark:border-orange-800"]
            } else {
                vec!["bg-white", "dark:bg-gray-700", "border", "border-gray-300", "dark:border-gray-600", 
                     "hover:border-blue-400", "dark:hover:border-blue-500", "shadow-sm"]
            }
        )}
        style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", x, y, w, h)}
    >
        { if is_phantom {
            // Show key name for phantom keys (since no binding)
            html! { <span class="text-[8px] text-gray-400 dark:text-gray-500">{&pk.name}</span> }
        } else {
            // Normal binding display
            html! {
                <>
                    <div class="w-full flex justify-between px-1 text-[7px] text-gray-400 absolute top-0.5">
                        <span class="truncate max-w-[45%]">{parts.top_left}</span>
                        <span class="truncate max-w-[45%] text-right">{parts.top_right}</span>
                    </div>
                    <span class="text-[12px] font-bold truncate px-1 mt-1">{parts.center}</span>
                </>
            }
        }}
        
        // Jump mode hint
        { if show_hint { /* ... */ } else { html! {} }}
    </div>
}
```

#### Update jump mode handling

Phantom keys should be included in the hint system:

```rust
// The hint system already uses physical_layout.len(), which now includes phantoms
let num_keys = props.data.physical_layout.len();

// Just need to handle the case where a phantom key is selected
let on_key_click = {
    let selected_key = props.selected_key.clone();
    Callback::from(move |key_index: usize| {
        let pk = &props.data.physical_layout[key_index];
        
        // For phantom keys, we need special handling
        if pk.origin == KeyOrigin::Phantom {
            // Will need to add to defsrc on save
            selected_key.set(Some(SelectedKey {
                layer_index: *current_layer,
                key_index,
                is_phantom: true,  // NEW field
            }));
        } else {
            selected_key.set(Some(SelectedKey {
                layer_index: *current_layer,
                key_index,
                is_phantom: false,
            }));
        }
    })
};
```

#### Update `SelectedKey` struct

```rust
#[derive(Clone, PartialEq, Debug)]
pub struct SelectedKey {
    pub layer_index: usize,
    pub key_index: usize,
    #[serde(default)]
    pub is_phantom: bool,  // NEW: indicates this was a phantom key
}
```

#### Update `KanataBindingPopup` for phantom key handling

```rust
fn KanataBindingPopup(props: &PopupProps) -> Html {
    let is_phantom = props.selected_key.is_phantom;
    
    let on_save = {
        Callback::from(move |e: MouseEvent| {
            if !is_valid { return; }
            
            let mut new_data = data.clone();
            
            if is_phantom {
                // This was a phantom key - need to:
                // 1. Add to defsrc at proper position
                // 2. Update all layer bindings (add entry for new key)
                // 3. Remove from phantom_keys list
                // 4. Update physical_layout origin
                
                let key_name = new_data.physical_layout[sk.key_index].name.clone();
                
                // Add to defsrc
                new_data.defsrc.push(key_name.clone());
                new_data.defsrc.sort(); // Or preserve standard layout order
                
                // Add binding entry to all layers
                for layer in &mut new_data.layers {
                    layer.bindings.push(text.clone());
                }
                
                // Remove from phantom_keys
                new_data.phantom_keys.retain(|p| p.name != key_name);
                
                // Update origin
                new_data.physical_layout[sk.key_index].origin = KeyOrigin::Standard;
                
                // Recompute layout to ensure proper ordering
                new_data.physical_layout = compute_standard_kanata_layout_with_phantoms(
                    &new_data.defsrc,
                    &new_data.unmapped_names,
                    &new_data.phantom_keys,
                    &sorted_alias_names,
                    is_mac, is_laptop
                );
            } else {
                // Normal save
                new_data.layers[sk.layer_index].bindings[sk.key_index] = text.clone();
            }
            
            on_update.emit(new_data);
            on_close.emit(e);
        })
    };
    
    // ... rest of popup
}
```

### 4. Save/Serialization Changes

When saving with phantom key modifications, the server needs to:

1. Update `defsrc` to include the new key
2. Update all `deflayer` blocks to include the new binding
3. Handle the key position in `defsrc` based on standard layout ordering

```rust
fn generate_kanata_kbd_with_phantoms(
    original: &str,
    data: &KeymapData,
    modified_phantoms: &[ModifiedPhantom],  // Track which phantoms were modified
) -> Result<String> {
    let mut content = original.to_string();
    
    // First, handle any phantom key insertions into defsrc
    for phantom in modified_phantoms {
        insert_key_into_defsrc(&mut content, &phantom.name, phantom.position)?;
        
        // Add binding to all deflayer blocks
        add_binding_to_all_layers(&mut content, &phantom.binding)?;
    }
    
    // Then proceed with normal binding updates
    // ... existing generate_kanata_kbd logic
    
    Ok(content)
}
```

### 5. Process-Unmapped-Keys Consideration

When `process-unmapped-keys=yes`, phantom keys behavior changes:

```rust
// In parse_kanata_with_tree_sitter:
let should_show_phantoms = match process_unmapped_keys {
    ProcessUnmappedKeys::Yes => false,  // All keys already available, no phantoms needed
    ProcessUnmappedKeys::No => true,     // Show phantoms
    ProcessUnmappedKeys::AllExcept(_) => true,  // Show phantoms (except excluded)
};

let phantom_keys = if should_show_phantoms {
    compute_phantom_keys(&key_names, is_mac, is_laptop)
} else {
    vec![]
};
```

## Test Cases

### Test 1: Basic Phantom Key Display

```scheme
(defsrc
  esc  a    b
)
```

**Expected:**
- `esc`, `a`, `b` shown as normal keys
- All other standard keys (1, 2, q, w, etc.) shown as phantom keys (outlined)
- Phantom keys positioned correctly in standard 108 layout

### Test 2: Phantom Key Click and Save

1. User clicks phantom key `f1`
2. Binding popup opens with empty binding
3. User enters `volume-up`
4. User clicks Save

**Expected:**
- `f1` added to `defsrc` after `esc` (standard layout order)
- All layers updated with new binding slot
- `f1` now appears as a regular key (filled)
- Generated .kbd file has updated `defsrc` and `deflayer` blocks

### Test 3: Jump Mode with Phantom Keys

```scheme
(defsrc a b)
```

**Expected:**
- Press `j` to activate jump mode
- `a`, `b` get hints "aa", "ab"
- Phantom keys get subsequent hints "ac", "ad", etc.
- Typing hint for phantom key opens binding popup

### Test 4: Process-Unmapped-Keys Yes

```scheme
(defcfg
  process-unmapped-keys yes
)

(defsrc a b)
```

**Expected:**
- NO phantom keys shown (all keys already available)
- Only `a`, `b` shown as regular keys
- Other keys accessible via Kanata's unmapped key handling

### Test 5: Round-Trip with Phantom Addition

1. Parse original file with partial defsrc
2. Modify a phantom key through UI
3. Save file
4. Re-parse saved file

**Expected:**
- Re-parsed file shows the formerly-phantom key as a regular key
- All bindings preserved correctly
- Physical layout updated to reflect new key

### Test 6: Multiple Phantom Key Additions

1. Add `f1` as phantom key
2. Add `f2` as phantom key  
3. Add `calc` as phantom key

**Expected:**
- All three keys added to `defsrc` in correct standard layout positions
- Layer bindings vector extended correctly for all layers
- Keys appear in correct visual order

### Test 7: Phantom Key with Alias

1. Click phantom key `caps`
2. Enter `@cap1` (reference to existing alias)
3. Save

**Expected:**
- `caps` added to `defsrc`
- All layers get `@cap1` binding at correct position
- Alias reference resolved correctly

## Implementation Plan

### Phase 1: Data Model Updates
1. Add `PhantomKey` struct
2. Add `KeyOrigin` enum  
3. Extend `PhysicalKey` with `origin` and `name` fields
4. Add `phantom_keys` field to `KeymapData`
5. Update serialization/deserialization

### Phase 2: Server-Side Parsing
1. Implement `compute_phantom_keys()` function
2. Update `compute_standard_kanata_layout()` to include phantoms
3. Modify `parse_kanata_with_tree_sitter()` to compute and return phantoms
4. Handle `process-unmapped-keys` logic for phantom visibility

### Phase 3: Server-Side Serialization
1. Implement `insert_key_into_defsrc()` function
2. Implement `add_binding_to_all_layers()` function
3. Update `generate_kanata_kbd()` to handle phantom key additions
4. Add proper position-based insertion in defsrc

### Phase 4: Frontend Rendering
1. Update `PhysicalKey` struct with new fields
2. Modify `KanataRenderer` to render phantom keys differently
3. Add CSS classes for phantom key styling
4. Ensure phantom keys show their name (not binding)

### Phase 5: Frontend Interaction
1. Update `SelectedKey` to track phantom status
2. Modify click handler to open popup for phantom keys
3. Update jump mode to include phantom keys in hints
4. Update hint display for phantom keys

### Phase 6: Save Logic
1. Update `KanataBindingPopup` save handler
2. Implement client-side data transformation for phantom->standard
3. Ensure proper layer binding synchronization
4. Handle physical_layout re-sorting

### Phase 7: Testing
1. Unit tests for `compute_phantom_keys()`
2. Unit tests for layout computation with phantoms
3. Integration tests for parse-render-save cycle
4. E2E tests for phantom key interaction flow
5. Round-trip tests for various configurations

## Files to Modify

### Server-Side
- `server/src/main.rs`:
  - Add `compute_phantom_keys()`
  - Update `parse_kanata_with_tree_sitter()`
  - Update `generate_kanata_kbd()`
  - Add `insert_key_into_defsrc()`
  - Add `add_binding_to_all_layers()`

### Shared (src/keymap/mod.rs)
- Add `PhantomKey` struct
- Add `KeyOrigin` enum
- Extend `PhysicalKey` struct
- Extend `KeymapData` struct
- Update `compute_standard_kanata_layout()` or add `compute_standard_kanata_layout_with_phantoms()`

### Frontend (src/kanata/mod.rs)
- Update `SelectedKey` struct
- Update `KanataRenderer` rendering logic
- Update `KanataBindingPopup` save handling
- Update jump mode hint generation
- Update click handlers

### Styling
- Add phantom key CSS classes (likely using Tailwind classes in the component)

## Backward Compatibility

- `KeyOrigin` defaults to `Standard` for existing data
- `phantom_keys` field defaults to empty vector
- Existing Kanata files without phantoms work unchanged
- Frontend gracefully handles missing phantom data from older server responses

## Future Enhancements

1. **Bulk phantom addition**: Allow selecting multiple phantom keys via checkbox mode
2. **Phantom key highlighting**: Highlight all phantom keys temporarily with a hotkey
3. **Smart defsrc ordering**: Option to preserve user's defsrc order vs standard layout order
4. **Phantom key search**: Filter phantom keys by name
5. **Keyboard layout selector**: Allow switching between standard/Mac/laptop layouts in UI
