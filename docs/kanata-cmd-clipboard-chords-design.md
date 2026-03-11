# Kanata cmd, Clipboard, and defchordsv2 Design Document

## Overview
Implement three features for the Kanata tab:
1. **cmd action** - Execute arbitrary binaries with arguments
2. **Clipboard actions** - Manipulate OS clipboard with save IDs
3. **defchordsv2** - Input chords/combos v2 configuration

---

## 1. cmd Action

### Kanata Reference
From Kanata docs:

```
(cmd $binary $arg1 $arg2 ... $argN)
(cmd-log $stdout-log-level $stderr-log-level)
(cmd-output-keys $binary $arg1 $arg2 ... $argN)
```

**Parameters:**
- `$binary`: Executable binary to run
- `$arg`: Arguments (strings, can be quoted or unquoted)
- `$stdout-log-level`/`$stderr-log-level`: One of `debug`, `info`, `warn`, `error`, `none`

**Examples:**
- `(cmd bazel build -c opt //...)` - Unquoted arguments
- `(cmd echo "hello world")` - Quoted argument with spaces
- `(cmd wtype á)` - Type accented character on Wayland
- `(cmd-log info error)` - Set log levels
- `(cmd-output-keys xclip -o)` - Output keys from command stdout

### Implementation Design

#### 1.1 New ParamType
Add `ParamType::String` for cmd arguments:
```rust
enum ParamType {
    Timeout,
    Integer,
    Layer,
    Action,
    Any,
    String,  // NEW: For cmd arguments
}
```

#### 1.2 Action Definitions
```rust
// cmd with variadic string params
KanataActionInfo {
    name: "cmd",
    params: &[ParamType::String],  // Variadic - at least 1 required
    description: "Execute a binary with arguments.",
}

// cmd-log with 2 specific string params
KanataActionInfo {
    name: "cmd-log",
    params: &[ParamType::String, ParamType::String],
    description: "Set stdout/stderr log levels (debug|info|warn|error|none).",
}

// cmd-output-keys with variadic string params
KanataActionInfo {
    name: "cmd-output-keys",
    params: &[ParamType::String],
    description: "Execute command and parse stdout as keys to type.",
}
```

#### 1.3 Validation
- `cmd`: At least 1 argument (binary name)
- `cmd-log`: Exactly 2 arguments, both must be valid log levels
- `cmd-output-keys`: At least 1 argument
- Strings can contain spaces if quoted

#### 1.4 Completion
- Suggest common commands: `echo`, `ls`, `pwd`, `wtype`, `powershell`, `xclip`
- For `cmd-log`: suggest log levels

---

## 2. Clipboard Actions

### Kanata Reference

```
(clipboard-set   $clipboard-string)
(clipboard-save  $save-id)
(clipboard-restore    $save-id)
(clipboard-save-swap  $save-id $save-id)
(clipboard-cmd-set  $binary $arg1 $arg2 ... $argN)
(clipboard-save-cmd-set  $save-id $binary $arg1 $arg2 ... $argN)
```

**Parameters:**
- `$clipboard-string`: Fixed string to set clipboard to
- `$save-id`: Number 0-65535 representing save slot
- `$binary`/`$arg`: Command to execute

**Examples:**
- `(clipboard-set "hello world")` - Set clipboard directly
- `(clipboard-save 0)` - Save current clipboard to slot 0
- `(clipboard-restore 0)` - Restore from slot 0
- `(clipboard-save-swap 0 1)` - Swap slots 0 and 1
- `(clipboard-cmd-set echo hello)` - Set from command output
- `(clipboard-save-cmd-set 0 powershell.exe -c "Get-Clipboard")`

### Implementation Design

#### 2.1 New ParamType
```rust
enum ParamType {
    // ... existing ...
    ClipboardId,  // NEW: For save IDs (0-65535)
}
```

#### 2.2 Action Definitions
```rust
KanataActionInfo {
    name: "clipboard-set",
    params: &[ParamType::String],
    description: "Set clipboard to string.",
}
KanataActionInfo {
    name: "clipboard-save",
    params: &[ParamType::ClipboardId],
    description: "Save clipboard to ID (0-65535).",
}
KanataActionInfo {
    name: "clipboard-restore",
    params: &[ParamType::ClipboardId],
    description: "Restore clipboard from ID.",
}
KanataActionInfo {
    name: "clipboard-save-swap",
    params: &[ParamType::ClipboardId, ParamType::ClipboardId],
    description: "Swap two clipboard save IDs.",
}
KanataActionInfo {
    name: "clipboard-cmd-set",
    params: &[ParamType::String],  // Variadic
    description: "Set clipboard from command output.",
}
KanataActionInfo {
    name: "clipboard-save-cmd-set",
    params: &[ParamType::ClipboardId, ParamType::String],  // ID + variadic
    description: "Set save ID content from command output.",
}
```

#### 2.3 Validation
- `clipboard-save`, `clipboard-restore`: ID must be 0-65535
- `clipboard-save-swap`: Both IDs must be 0-65535
- `clipboard-cmd-set`: At least 1 argument (binary)
- `clipboard-save-cmd-set`: At least 2 arguments (ID + binary)

#### 2.4 Completion
- Suggest save IDs: 0, 1, 2, 3, 4, 5
- For commands: suggest same as cmd action

---

## 3. defchordsv2

### Kanata Reference

```
(defchordsv2
  (participating-keys1) action1 timeout1 release-behaviour1 (disabled-layers1)
  ...
  (participating-keysN) actionN timeoutN release-behaviourN (disabled-layersN)
)
```

**5-tuple per chord:**
1. `$participating-keys`: List of 2+ key names (defsrc-compatible)
2. `$action`: Action to activate when chord triggers
3. `$timeout`: Milliseconds within which all keys must be pressed
4. `$release-behaviour`: `first-release` or `all-released`
5. `$disabled-layers`: List of layer names where chord is disabled

**Requirements:**
- Must enable `concurrent-tap-hold` in defcfg
- Minimum 2 keys per chord
- Unique key list per chord

**Example:**
```
(defcfg concurrent-tap-hold yes)
(defchordsv2
  (a s)    c                200 all-released  (non-chord-layer)
  (a s d) (macro h e l l o) 250 first-release (non-chord-layer)
  (s d f) (macro b y e)     400 first-release ()
)
```

### Implementation Design

#### 3.1 New Types
```rust
#[derive(Debug, Clone)]
struct ChordV2 {
    keys: Vec<String>,           // 2+ participating keys
    action: String,              // Action to activate
    timeout: u32,                // Milliseconds
    release_behaviour: ReleaseBehaviour,  // first-release | all-released
    disabled_layers: Vec<String>, // Layer names
}

#[derive(Debug, Clone, PartialEq)]
enum ReleaseBehaviour {
    FirstRelease,
    AllReleased,
}
```

#### 3.2 KeymapData Extension
```rust
struct KeymapData {
    // ... existing fields ...
    pub chordsv2: Vec<ChordV2>,  // NEW
}
```

#### 3.3 Parsing defchordsv2
The defchordsv2 block contains 5-tuples. Each tuple:
1. `(key1 key2 ...)` - Parenthesized list of keys
2. `action` - Action string or nested action
3. `timeout` - Integer
4. `release-behaviour` - String: `first-release` or `all-released`
5. `(layer1 layer2 ...)` or `()` - Disabled layers list

#### 3.4 Validation
- Each chord must have ≥2 keys
- Keys must be valid defsrc keys
- Timeout must be positive integer
- Release behaviour must be valid enum value
- Disabled layers must exist in layers list
- No duplicate key combinations

#### 3.5 Completion
- After `(defchordsv2`, suggest key list start `(`
- After key list, suggest common actions
- After action, suggest timeout values: 100, 200, 300, 400, 500
- After timeout, suggest release behaviours
- After release behaviour, suggest layer list or `()`

---

## Test Plan

### cmd Action Tests

```rust
#[test]
fn test_cmd_actions_exist() {
    assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd"));
    assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd-log"));
    assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd-output-keys"));
}

#[test]
fn test_validate_cmd_simple() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(cmd echo hello)"));
    assert!(validator.validate_action("(cmd ls -la)"));
    assert!(validator.validate_action("(cmd bazel build -c opt //...)"));
}

#[test]
fn test_validate_cmd_quoted() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(cmd echo \"hello world\")"));
    assert!(validator.validate_action("(cmd powershell.exe -c \"Get-Date\")"));
}

#[test]
fn test_validate_cmd_log() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(cmd-log info error)"));
    assert!(validator.validate_action("(cmd-log debug none)"));
    assert!(validator.validate_action("(cmd-log warn info)"));
}

#[test]
fn test_validate_cmd_log_invalid() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Invalid log levels
    assert!(!validator.validate_action("(cmd-log invalid error)"));
    assert!(!validator.validate_action("(cmd-log info invalid)"));
    
    // Wrong number of params
    assert!(!validator.validate_action("(cmd-log info)"));
    assert!(!validator.validate_action("(cmd-log info error extra)"));
}

#[test]
fn test_validate_cmd_no_args() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // cmd requires at least binary name
    assert!(!validator.validate_action("(cmd)"));
}

#[test]
fn test_validate_cmd_output_keys() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(cmd-output-keys xclip -o)"));
    assert!(validator.validate_action("(cmd-output-keys echo hello)"));
}
```

### Clipboard Action Tests

```rust
#[test]
fn test_clipboard_actions_exist() {
    let actions = [
        "clipboard-set", "clipboard-save", "clipboard-restore",
        "clipboard-save-swap", "clipboard-cmd-set", "clipboard-save-cmd-set"
    ];
    for action in &actions {
        assert!(KANATA_ACTIONS.iter().any(|a| a.name == *action));
    }
}

#[test]
fn test_validate_clipboard_set() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(clipboard-set \"hello\")"));
    assert!(validator.validate_action("(clipboard-set hello)"));
}

#[test]
fn test_validate_clipboard_save_restore() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Valid IDs: 0-65535
    assert!(validator.validate_action("(clipboard-save 0)"));
    assert!(validator.validate_action("(clipboard-save 65535)"));
    assert!(validator.validate_action("(clipboard-restore 12345)"));
}

#[test]
fn test_validate_clipboard_save_invalid_id() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(!validator.validate_action("(clipboard-save -1)"));
    assert!(!validator.validate_action("(clipboard-save 65536)"));
    assert!(!validator.validate_action("(clipboard-save abc)"));
}

#[test]
fn test_validate_clipboard_save_swap() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(clipboard-save-swap 0 1)"));
    assert!(validator.validate_action("(clipboard-save-swap 100 200)"));
}

#[test]
fn test_validate_clipboard_cmd_set() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(clipboard-cmd-set echo hello)"));
    assert!(validator.validate_action("(clipboard-cmd-set powershell.exe -c Get-Date)"));
}

#[test]
fn test_validate_clipboard_save_cmd_set() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(clipboard-save-cmd-set 0 echo hello)"));
    assert!(validator.validate_action("(clipboard-save-cmd-set 5 powershell.exe -c Get-Date)"));
    
    // Needs at least ID + binary
    assert!(!validator.validate_action("(clipboard-save-cmd-set 0)"));
}

#[test]
fn test_clipboard_in_macro() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Common use case: save, do something, restore
    assert!(validator.validate_action(
        "(macro (clipboard-save 0) 20 C-v (clipboard-restore 0))"
    ));
}
```

### defchordsv2 Tests

```rust
#[test]
fn test_chordv2_parsing() {
    let input = r#"
(defchordsv2
  (a s) c 200 all-released ()
  (s d) (macro x y) 300 first-release (layer1)
)
"#;
    let chords = parse_defchordsv2(input).unwrap();
    assert_eq!(chords.len(), 2);
    
    assert_eq!(chords[0].keys, vec!["a", "s"]);
    assert_eq!(chords[0].action, "c");
    assert_eq!(chords[0].timeout, 200);
    assert_eq!(chords[0].release_behaviour, ReleaseBehaviour::AllReleased);
    assert!(chords[0].disabled_layers.is_empty());
    
    assert_eq!(chords[1].keys, vec!["s", "d"]);
    assert_eq!(chords[1].action, "(macro x y)");
    assert_eq!(chords[1].timeout, 300);
    assert_eq!(chords[1].release_behaviour, ReleaseBehaviour::FirstRelease);
    assert_eq!(chords[1].disabled_layers, vec!["layer1"]);
}

#[test]
fn test_chordv2_minimum_keys() {
    let data = create_test_data();
    
    // Must have at least 2 keys
    assert!(validate_chord(&ChordV2 {
        keys: vec!["a".to_string(), "s".to_string()],
        action: "c".to_string(),
        timeout: 200,
        release_behaviour: ReleaseBehaviour::AllReleased,
        disabled_layers: vec![],
    }, &data));
    
    // Single key is invalid
    assert!(!validate_chord(&ChordV2 {
        keys: vec!["a".to_string()],
        action: "c".to_string(),
        timeout: 200,
        release_behaviour: ReleaseBehaviour::AllReleased,
        disabled_layers: vec![],
    }, &data));
}

#[test]
fn test_chordv2_timeout_validation() {
    let data = create_test_data();
    
    assert!(!validate_chord(&ChordV2 {
        keys: vec!["a".to_string(), "s".to_string()],
        action: "c".to_string(),
        timeout: 0,  // Invalid
        release_behaviour: ReleaseBehaviour::AllReleased,
        disabled_layers: vec![],
    }, &data));
}

#[test]
fn test_chordv2_duplicate_keys() {
    let data = create_test_data();
    let chords = vec![
        ChordV2 {
            keys: vec!["a".to_string(), "s".to_string()],
            action: "c".to_string(),
            timeout: 200,
            release_behaviour: ReleaseBehaviour::AllReleased,
            disabled_layers: vec![],
        },
        ChordV2 {
            keys: vec!["a".to_string(), "s".to_string()],  // Same keys!
            action: "d".to_string(),
            timeout: 300,
            release_behaviour: ReleaseBehaviour::FirstRelease,
            disabled_layers: vec![],
        },
    ];
    
    // Duplicate key combinations should be invalid
    assert!(!validate_chordsv2(&chords, &data));
}

#[test]
fn test_chordv2_disabled_layers() {
    let mut data = create_test_data();
    data.layers.push(Layer {
        name: "special".to_string(),
        bindings: vec![],
        layer_type: LayerType::Deflayer,
        source_layer: None,
        key_bindings: HashMap::new(),
    });
    
    // Valid: disabled layer exists
    assert!(validate_chord(&ChordV2 {
        keys: vec!["a".to_string(), "s".to_string()],
        action: "c".to_string(),
        timeout: 200,
        release_behaviour: ReleaseBehaviour::AllReleased,
        disabled_layers: vec!["special".to_string()],
    }, &data));
    
    // Invalid: disabled layer doesn't exist
    assert!(!validate_chord(&ChordV2 {
        keys: vec!["a".to_string(), "s".to_string()],
        action: "c".to_string(),
        timeout: 200,
        release_behaviour: ReleaseBehaviour::AllReleased,
        disabled_layers: vec!["nonexistent".to_string()],
    }, &data));
}
```

### Integration Tests

```rust
#[test]
fn test_complex_config_with_all_features() {
    let config = r#"
(defcfg
  danger-enable-cmd yes
  concurrent-tap-hold yes
)

(defsrc a s d f)

(defchordsv2
  (a s) (cmd echo "chord triggered") 200 first-release ()
  (s d) (clipboard-set "hello") 300 all-released ()
)

(defalias
  clip-save (macro (clipboard-save 0) 100 C-v (clipboard-restore 0))
  run-cmd (cmd echo hello world)
  set-clip (clipboard-cmd-set echo "from command")
)

(deflayer base
  @run-cmd a s @clip-save
)
"#;
    
    let result = parse_kanata_config(config);
    assert!(result.is_ok());
}
```

---

## Implementation Status

✅ **COMPLETED** - All features implemented and tested.

### Phase 1: cmd Action ✅
- [x] Added `ParamType::String`
- [x] Added `cmd`, `cmd-log`, `cmd-output-keys` to KANATA_ACTIONS
- [x] Updated validation for variadic string params
- [x] Added validation for log levels (`debug`, `info`, `warn`, `error`, `none`)
- [x] Added comprehensive tests

### Phase 2: Clipboard Actions ✅
- [x] Added `ParamType::ClipboardId`
- [x] Added all 6 clipboard actions to KANATA_ACTIONS:
  - `clipboard-set`, `clipboard-save`, `clipboard-restore`
  - `clipboard-save-swap`, `clipboard-cmd-set`, `clipboard-save-cmd-set`
- [x] Updated validation for clipboard ID range (0-65535)
- [x] Added comprehensive tests

### Phase 3: defchordsv2 Types ✅
- [x] Added `ChordV2` struct with fields: `keys`, `action`, `timeout`, `release_behaviour`, `disabled_layers`
- [x] Added `ReleaseBehaviour` enum (`FirstRelease`, `AllReleased`)
- [x] Added `chordsv2` field to `KeymapData`
- [x] Added comprehensive tests for types and validation logic

### Phase 4: Completion Enhancement (Future Enhancement)
- [ ] Add completion for cmd arguments (common commands)
- [ ] Add completion for clipboard IDs (0-5)
- [ ] Add completion for defchordsv2 fields

### Phase 5: Integration Testing ✅
- [x] Tested cmd actions with various arguments
- [x] Tested clipboard actions with valid/invalid IDs
- [x] Tested integration with `multi` and `macro`
- [x] Edge cases covered (empty args, invalid IDs, etc.)
