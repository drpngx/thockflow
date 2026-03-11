# Design and Implementation Document: Kanata `defvar` Support

**Status:** ✅ Implemented  
**Date:** 2026-03-11

## Overview

This document describes the implementation of kanata's `(defvar)` feature in ThockFlow. The `defvar` construct allows defining variables with typed values that can be referenced elsewhere in the configuration using the `$variable-name` syntax.

## Feature Description

### What is `defvar`?

From the [kanata documentation](https://github.com/jtroo/kanata/blob/main/docs/config.adoc#variables):

```scheme
(defvar tap-timeout 100)
(defvar hold-timeout 200)
(defvar home-row-mod (tap-hold $tap-timeout $hold-timeout a lctl))
```

Unlike `defalias` which creates action aliases referenced with `@`, `defvar` creates variable substitutions referenced with `$`. The key difference:

- **`defalias`**: `@alias-name` → substitutes the entire action expression
- **`defvar`**: `$var-name` → substitutes the value inline (can be used within expressions)

### Variable Types

Based on kanata's documentation and common usage patterns:

| Type | Example | Description |
|------|---------|-------------|
| **Integer** | `(defvar tap-timeout 100)` | Numeric values for timeouts, delays |
| **Key/Action** | `(defvar my-key lctl)` | Single key or action reference |
| **List** | `(defvar my-keys (a b c))` | List of keys for use with `macro`, `multi`, etc. |
| **String** | `(defvar my-str "hello")` | String values (less common) |

### Type-Aware Completion

The key feature of this implementation is **type-aware completion**. When typing `(tap-hold x y z zz)`:

- Position 1 (`x`): Suggests integer variables (e.g., `$tap-timeout`, `$hold-timeout`)
- Position 2 (`y`): Suggests integer variables
- Position 3 (`z`): Suggests action/key variables (e.g., `$my-key`)
- Position 4 (`zz`): Suggests action/key variables

## Implementation Design

### Data Model Changes

#### 1. New `VarType` Enum

```rust
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum VarType {
    Integer,
    Key,           // Single key or action
    Action,        // Complex action expression
    List,          // List of items (a b c)
    String,
    Unknown,       // Couldn't determine type
}
```

#### 2. New `Defvar` Struct

```rust
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Defvar {
    pub name: String,        // Variable name (without $ prefix)
    pub value: String,       // Raw value as string
    pub var_type: VarType,   // Auto-detected type
}
```

#### 3. Extend `KeymapData` to Store Variables

```rust
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct KeymapData {
    pub physical_layout: Vec<PhysicalKey>,
    pub layers: Vec<Layer>,
    pub includes: Vec<String>,
    pub aliases: HashMap<String, String>,
    pub defsrc: Vec<String>,
    pub unmapped_names: Vec<String>,
    pub process_unmapped_keys: ProcessUnmappedKeys,
    // NEW field for defvar support
    #[serde(default)]
    pub defvars: Vec<Defvar>,
}
```

### Type Detection Algorithm

The type of a variable value is auto-detected based on its content:

```rust
fn detect_var_type(value: &str) -> VarType {
    let trimmed = value.trim();
    
    // Integer: pure digits, optionally with sign
    if trimmed.parse::<i64>().is_ok() {
        return VarType::Integer;
    }
    
    // String: wrapped in quotes
    if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
       (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
        return VarType::String;
    }
    
    // List: wrapped in parentheses with multiple items
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len()-1];
        let parts: Vec<&str> = inner.split_whitespace().collect();
        if parts.len() > 1 {
            return VarType::List;
        }
        // Single item in parens might be an action
        return VarType::Action;
    }
    
    // Single key or action reference
    if is_kanata_key(trimmed) || trimmed.starts_with('@') {
        return VarType::Key;
    }
    
    VarType::Unknown
}
```

### Parsing Logic

#### Parse `defvar` Nodes

Add parsing logic similar to `defalias` in `parse_kanata_with_tree_sitter()`:

```rust
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
```

### Completion System Updates

#### Update Parameter Type Definitions

Extend the `ParamType` enum in `src/kanata/mod.rs`:

```rust
#[derive(Clone, Copy, PartialEq)]
enum ParamType {
    Timeout,       // Integer for timeouts
    Action,        // Single action or key
    Layer,         // Layer name
    Any,           // Any value
    Integer,       // NEW: Generic integer
    Variable(VarType), // NEW: Typed variable reference
}
```

#### Update Action Definitions

Update `KANATA_ACTIONS` to include more precise type information:

```rust
static KANATA_ACTIONS: &[KanataActionInfo] = &[
    KanataActionInfo {
        name: "tap-hold",
        params: &[
            ParamType::Integer,  // tap timeout
            ParamType::Integer,  // hold timeout
            ParamType::Action,   // tap action
            ParamType::Action,   // hold action
        ],
        description: "Tap for one action, hold for another.",
    },
    // ... other actions
];
```

#### Update `get_suggestions` Function

Modify the completion logic to suggest variables based on expected type:

```rust
fn get_suggestions(text: &str, data: &KeymapData) -> (String, Vec<String>) {
    // ... existing logic ...
    
    // Determine expected type for current parameter position
    let expected_type = get_expected_param_type(&lower_text);
    
    // Suggest variables matching the expected type
    if let Some(expected) = expected_type {
        for defvar in &data.defvars {
            if matches_type(&defvar.var_type, &expected) {
                let suggestion = format!("${}", defvar.name);
                if suggestion.contains(query) {
                    suggestions.push(suggestion);
                }
            }
        }
    }
    
    // ... rest of existing logic ...
}

fn get_expected_param_type(text: &str) -> Option<ParamType> {
    // Parse the current context to determine what type is expected
    // e.g., if we're at position 1 in (tap-hold ...), return Integer
    
    if let Some(last_open) = text.rfind('(') {
        let after_open = &text[last_open + 1..];
        let parts: Vec<&str> = after_open.split_whitespace().collect();
        
        if parts.is_empty() {
            return None;
        }
        
        if let Some(action) = KANATA_ACTIONS.iter().find(|a| a.name == parts[0]) {
            let param_idx = if after_open.ends_with(' ') {
                parts.len() - 1
            } else {
                parts.len().saturating_sub(2)
            };
            return action.params.get(param_idx).copied();
        }
    }
    
    None
}

fn matches_type(var_type: &VarType, param_type: &ParamType) -> bool {
    match (var_type, param_type) {
        (VarType::Integer, ParamType::Timeout) => true,
        (VarType::Integer, ParamType::Integer) => true,
        (VarType::Key, ParamType::Action) => true,
        (VarType::Action, ParamType::Action) => true,
        (VarType::List, ParamType::Any) => true,
        _ => false,
    }
}
```

### Variable Substitution (Future Consideration)

While the primary focus is on completion, we may want to track where variables are used:

```rust
// In Layer or binding tracking
pub struct VariableUsage {
    pub var_name: String,
    pub location: (usize, usize), // line, column
    pub context: String,          // surrounding text
}
```

This would enable features like "find all references" or "rename variable".

## Test Cases

### Test 1: Parse Integer Variable

```scheme
(defvar tap-timeout 100)
(defvar hold-timeout 200)

(defalias
  th (tap-hold $tap-timeout $hold-timeout a lctl)
)
```

**Expected:**
- `defvars` contains: `[Defvar{name: "tap-timeout", value: "100", var_type: Integer}, Defvar{name: "hold-timeout", value: "200", var_type: Integer}]`
- Aliases correctly reference variables (stored as strings with `$` prefix)

### Test 2: Parse Key Variable

```scheme
(defvar my-mod lctl)
(defvar my-key a)

(deflayer base
  $my-mod $my-key
)
```

**Expected:**
- `defvars` contains variables with `var_type: Key`

### Test 3: Parse List Variable

```scheme
(defvar my-macro (a b c d))

(defalias
  mm (macro $my-macro)
)
```

**Expected:**
- Variable has `var_type: List`

### Test 4: Integer Completion in tap-hold

Given:
```scheme
(defvar tap-timeout 100)
(defvar hold-timeout 200)
(defvar my-key a)
```

When typing: `(tap-hold `

**Expected:** Suggestions include `$tap-timeout`, `$hold-timeout` but NOT `$my-key`

### Test 5: Action Completion in tap-hold

Given:
```scheme
(defvar tap-timeout 100)
(defvar my-key a)
(defvar my-action (layer-toggle nav))
```

When typing: `(tap-hold 200 300 `

**Expected:** Suggestions include `$my-key`, `$my-action` but NOT `$tap-timeout`

### Test 6: Mixed Completion

Given:
```scheme
(defvar timeout1 100)
(defvar timeout2 200)
(defvar key1 a)
(defvar key2 b)
```

When typing: `(tap-hold $`

**Expected:** Suggestions include all variables starting with `$`, but when continuing:
- After `(tap-hold $timeout1 ` → suggest only integer variables for position 2
- After `(tap-hold $timeout1 $timeout2 ` → suggest only action/key variables for position 3

### Test 7: Variable in deflayermap

```scheme
(defvar nav-toggle (layer-toggle nav))

(deflayermap (base)
  a $nav-toggle
)
```

**Expected:**
- Variable is correctly parsed and available in completion

### Test 8: Empty Variable Value (Error Case)

```scheme
(defvar empty-var)
```

**Expected:** Variable is either skipped or stored with `var_type: Unknown`

### Test 9: Complex Nested Variable

```scheme
(defvar base-tap 150)
(defvar base-hold 200)
(defvar home-row-mod
  (tap-hold $base-tap $base-hold a lctl)
)
```

**Expected:**
- Each variable is parsed independently
- `home-row-mod` has `var_type: Action`
- No circular dependency checking at this stage

### Test 10: Round-trip Serialization

1. Parse a file with `defvar` definitions
2. Generate output
3. Verify `defvar` blocks are preserved

**Expected:** Original defvar definitions are maintained in generated output

## Implementation Plan

### Phase 1: Data Model
1. Add `VarType` enum
2. Add `Defvar` struct
3. Add `defvars` field to `KeymapData`

### Phase 2: Parsing
1. Add `find_kanata_node` call for `defvar` in `parse_kanata_with_tree_sitter()`
2. Implement `detect_var_type()` function
3. Store parsed variables in `KeymapData`

### Phase 3: Type-Aware Completion
1. Update `ParamType` enum with `Integer` and `Variable` variants
2. Update `KANATA_ACTIONS` with more precise type information
3. Modify `get_suggestions()` to filter variables by type
4. Add `get_expected_param_type()` helper function

### Phase 4: Testing
1. Add parsing tests for each variable type
2. Add completion tests for type filtering
3. Add integration tests for end-to-end scenarios

## Files to Modify

1. **`src/keymap/mod.rs`**:
   - Add `VarType` enum
   - Add `Defvar` struct
   - Extend `KeymapData` with `defvars` field

2. **`server/src/main.rs`**:
   - Update `parse_kanata_with_tree_sitter()` to parse `defvar` nodes
   - Implement `detect_var_type()` function
   - Include defvars in parsed `KeymapData`

3. **`src/kanata/mod.rs`**:
   - Update `ParamType` enum
   - Update `KANATA_ACTIONS` definitions
   - Modify `get_suggestions()` for type-aware completion

## Backward Compatibility

- `defvars` field defaults to empty vector for existing code
- Variable substitution is not required for basic functionality
- Existing completion behavior should remain unchanged when no defvars exist

## Example Usage Flow

1. **User defines variables:**
   ```scheme
   (defvar tap-timeout 100)
   (defvar hold-timeout 200)
   (defvar home-key a)
   (defvar mod-key lctl)
   ```

2. **User types in editor:**
   ```
   (tap-hold 
   ```

3. **Completion shows:**
   - `$tap-timeout`
   - `$hold-timeout`
   - `100`, `200`, `50` (literal suggestions)
   
   But NOT:
   - `$home-key`
   - `$mod-key`

4. **User selects `$tap-timeout` and continues:**
   ```
   (tap-hold $tap-timeout 
   ```

5. **Completion shows again:**
   - `$tap-timeout`
   - `$hold-timeout`
   - `100`, `200`, `50`

6. **User fills both timeouts and types:**
   ```
   (tap-hold $tap-timeout $hold-timeout 
   ```

7. **Completion now shows:**
   - `$home-key`
   - `$mod-key`
   - All key suggestions (`a`, `b`, `lctl`, etc.)
   
   But NOT:
   - `$tap-timeout`
   - `$hold-timeout`

This provides an intelligent, context-aware editing experience that helps users write correct kanata configurations.

## Implementation Summary

### Completed Work

All phases of the implementation have been completed:

1. **Data Model Changes** (`src/keymap/mod.rs`):
   - Added `VarType` enum with variants: `Integer`, `Key`, `Action`, `List`, `String`, `Unknown`
   - Added `Defvar` struct with `name`, `value`, and `var_type` fields
   - Extended `KeymapData` with `defvars: Vec<Defvar>` field

2. **Parsing Implementation** (`server/src/main.rs`):
   - Added `detect_var_type()` function that auto-detects variable types based on value content
   - Added `defvar` node parsing in `parse_kanata_with_tree_sitter()` following the same pattern as `defalias`
   - Handles all variable types: integers, keys, actions, lists, and strings

3. **Type-Aware Completion** (`src/kanata/mod.rs`):
   - Extended `ParamType` enum with `Integer` variant
   - Updated `KANATA_ACTIONS` to use `ParamType::Integer` for timeout parameters
   - Added `get_expected_param_type()` to determine the expected type at current cursor position
   - Added `matches_type()` to check if a variable type matches the expected parameter type
   - Modified `get_suggestions()` to filter and suggest variables based on their types

4. **Tests** (both files):
   - `test_parse_defvar_integer` - Tests parsing integer variables
   - `test_parse_defvar_key` - Tests parsing key variables
   - `test_parse_defvar_list` - Tests parsing list variables
   - `test_parse_defvar_action` - Tests parsing action variables
   - `test_parse_defvar_mixed_types` - Tests parsing mixed variable types
   - `test_detect_var_type_edge_cases` - Tests edge cases in type detection
   - `test_defvar_in_deflayermap` - Tests variables in deflayermap context
   - `test_integer_completion_in_tap_hold` - Tests integer variable suggestions in timeout positions
   - `test_action_completion_in_tap_hold` - Tests action/key variable suggestions in action positions
   - `test_variable_prefix_completion` - Tests $ prefix filtering
   - `test_completion_with_variable_used` - Tests completion after using a variable
   - `test_get_expected_param_type` - Tests parameter type detection
   - `test_matches_type` - Tests type matching logic
   - `test_integer_completion_in_other_actions` - Tests integer completion in various actions

### Test Results

All tests pass:
- `//:thockflow_test` - Library tests including kanata completion tests
- `//server:server_test` - Server tests including defvar parsing tests

### Type Detection Behavior

The `detect_var_type()` function uses the following heuristics:
- **Integer**: Pure digits, optionally with leading minus sign (e.g., `100`, `-50`)
- **String**: Wrapped in single or double quotes (e.g., `"hello"`, `'world'`)
- **Action**: Parenthesized expression with known action name (e.g., `(layer-toggle nav)`, `(tap-hold 200 200 a lctl)`)
- **List**: Parenthesized with multiple space-separated items (e.g., `(a b c d)`)
- **Key**: Single word that looks like a key name or alias reference (e.g., `lctl`, `a`, `@my-alias`)
- **Unknown**: Anything that doesn't match the above patterns

### Completion Behavior Examples

| Context | Expected Type | Suggested Variables |
|---------|---------------|---------------------|
| `(tap-hold \|` | Integer | `$tap-timeout`, `$hold-timeout` |
| `(tap-hold 200 \|` | Integer | `$tap-timeout`, `$hold-timeout` |
| `(tap-hold 200 300 \|` | Action | `$my-key`, `$nav-toggle` |
| `(one-shot \|` | Integer | `$tap-timeout` |
| `(layer-toggle \|` | Layer | (layer names, not variables) |
