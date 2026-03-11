# Kanata cmd Action Design Document

## Overview
Add support for the Kanata `cmd` action which allows executing arbitrary binaries with arguments. This is a powerful feature that enables keyboard-driven automation.

## Kanata Reference

### cmd
Execute arbitrary binaries with arguments.

```
(cmd $binary $arg1 $arg2 ... $argN)
```

**Parameters:**
- `$binary`: Executable binary to run
- `$arg`: Arguments (strings, can be quoted or unquoted)

**Examples:**
```
(cmd echo hello)                          ; Simple echo
(cmd bazel build -c opt //...)           ; Unquoted arguments with special chars
(cmd powershell.exe -c "Get-Date")       ; Quoted argument with spaces
(cmd wtype á)                            ; Type accented character on Wayland
```

### cmd-log
Set log levels for cmd output.

```
(cmd-log $stdout-log-level $stderr-log-level)
```

**Parameters:**
- `$stdout-log-level`: One of `debug`, `info`, `warn`, `error`, `none`
- `$stderr-log-level`: One of `debug`, `info`, `warn`, `error`, `none`

**Examples:**
```
(cmd-log info error)      ; Info for stdout, error for stderr
(cmd-log debug none)      ; Debug for stdout, no stderr logging
```

### cmd-output-keys
Execute command and parse stdout as keys to type.

```
(cmd-output-keys $binary $arg1 $arg2 ... $argN)
```

**Examples:**
```
(cmd-output-keys xclip -o)         ; Paste clipboard content as keys
(cmd-output-keys echo hello)       ; Type "hello"
```

## Implementation Design

### 1. New ParamType
Add `ParamType::String` for cmd arguments:
```rust
enum ParamType {
    Timeout,
    Integer,
    Layer,
    Action,
    Any,
    String,  // NEW: For cmd arguments (quoted or unquoted)
}
```

### 2. Action Definitions
```rust
KanataActionInfo {
    name: "cmd",
    params: &[ParamType::String],  // Variadic - at least 1 required
    description: "Execute a binary with arguments.",
}

KanataActionInfo {
    name: "cmd-log",
    params: &[ParamType::String, ParamType::String],
    description: "Set cmd log levels. Params: stdout-level stderr-level",
}

KanataActionInfo {
    name: "cmd-output-keys",
    params: &[ParamType::String],  // Variadic - at least 1 required
    description: "Execute command and parse stdout as keys to type.",
}
```

### 3. Validation Logic

**cmd validation:**
- At least 1 argument required (binary name)
- Arguments can be any string (quoted or unquoted)
- Examples of valid commands:
  - `(cmd echo hello)` - Simple unquoted
  - `(cmd "echo" "hello world")` - Quoted with spaces
  - `(cmd bazel build -c opt //...)` - Unquoted with special chars

**cmd-log validation:**
- Exactly 2 arguments required
- Both must be valid log levels: `debug`, `info`, `warn`, `error`, `none`
- Case-insensitive matching

**cmd-output-keys validation:**
- At least 1 argument required (binary name)
- Same string validation as cmd

### 4. String Parsing
Strings in cmd can be:
1. **Unquoted**: Simple words without spaces
   - Example: `echo`, `hello`, `build`, `-c`
2. **Quoted**: Words with spaces or special characters
   - Example: `"hello world"`, `"Get-Date"`

The existing `split_parts` function handles this by:
- Splitting on whitespace at depth 0 (top level)
- Preserving quoted strings as single tokens
- Not evaluating escapes (handled by Kanata parser)

### 5. Completion System

**Action name completion:**
- Typing `(cm` → suggest `(cmd`, `(cmd-log`, `(cmd-output-keys`

**Argument completion (future enhancement):**
- After `(cmd ` → suggest common commands: `echo`, `ls`, `pwd`, `powershell`, `wtype`, `xclip`
- After `(cmd-log ` → suggest log levels: `debug`, `info`, `warn`, `error`, `none`

## Test Plan

### Unit Tests for Action Definitions

```rust
#[test]
fn test_cmd_actions_exist() {
    assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd"));
    assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd-log"));
    assert!(KANATA_ACTIONS.iter().any(|a| a.name == "cmd-output-keys"));
}

#[test]
fn test_cmd_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "cmd").unwrap();
    assert_eq!(action.params, &[ParamType::String]);
}

#[test]
fn test_cmd_log_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "cmd-log").unwrap();
    assert_eq!(action.params, &[ParamType::String, ParamType::String]);
}

#[test]
fn test_cmd_output_keys_params() {
    let action = KANATA_ACTIONS.iter().find(|a| a.name == "cmd-output-keys").unwrap();
    assert_eq!(action.params, &[ParamType::String]);
}
```

### Unit Tests for Validation

```rust
#[test]
fn test_validate_cmd_simple() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Simple unquoted arguments
    assert!(validator.validate_action("(cmd echo hello)"));
    assert!(validator.validate_action("(cmd ls -la)"));
}

#[test]
fn test_validate_cmd_unquoted_special_chars() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Unquoted arguments with special characters
    assert!(validator.validate_action("(cmd bazel build -c opt //...)"));
    assert!(validator.validate_action("(cmd gcc -O2 -Wall file.c)"));
    assert!(validator.validate_action("(cmd git commit -m \"message\")"));
}

#[test]
fn test_validate_cmd_quoted() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Quoted arguments with spaces
    assert!(validator.validate_action("(cmd echo \"hello world\")"));
    assert!(validator.validate_action("(cmd powershell.exe -c \"Get-Date\")"));
    assert!(validator.validate_action("(cmd wtype \"á\")"));
}

#[test]
fn test_validate_cmd_mixed() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Mix of quoted and unquoted
    assert!(validator.validate_action("(cmd bash -c \"echo hello\")"));
    assert!(validator.validate_action("(cmd python3 -c \"print(1+1)\")"));
}

#[test]
fn test_validate_cmd_no_args() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // cmd requires at least binary name
    assert!(!validator.validate_action("(cmd)"));
}

#[test]
fn test_validate_cmd_single_arg() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Single arg is valid (just binary name)
    assert!(validator.validate_action("(cmd ls)"));
    assert!(validator.validate_action("(cmd whoami)"));
}

#[test]
fn test_validate_cmd_log() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Valid log level combinations
    assert!(validator.validate_action("(cmd-log info error)"));
    assert!(validator.validate_action("(cmd-log debug none)"));
    assert!(validator.validate_action("(cmd-log warn info)"));
    assert!(validator.validate_action("(cmd-log error debug)"));
}

#[test]
fn test_validate_cmd_log_case_insensitive() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Case insensitive
    assert!(validator.validate_action("(cmd-log INFO ERROR)"));
    assert!(validator.validate_action("(cmd-log Debug None)"));
}

#[test]
fn test_validate_cmd_log_invalid_level() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Invalid log levels
    assert!(!validator.validate_action("(cmd-log invalid error)"));
    assert!(!validator.validate_action("(cmd-log info invalid)"));
    assert!(!validator.validate_action("(cmd-log foo bar)"));
}

#[test]
fn test_validate_cmd_log_wrong_arity() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Wrong number of params
    assert!(!validator.validate_action("(cmd-log)"));
    assert!(!validator.validate_action("(cmd-log info)"));
    assert!(!validator.validate_action("(cmd-log info error extra)"));
}

#[test]
fn test_validate_cmd_output_keys() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    assert!(validator.validate_action("(cmd-output-keys xclip -o)"));
    assert!(validator.validate_action("(cmd-output-keys echo hello)"));
    assert!(validator.validate_action("(cmd-output-keys cat file.txt)"));
}

#[test]
fn test_validate_cmd_output_keys_no_args() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Requires at least binary name
    assert!(!validator.validate_action("(cmd-output-keys)"));
}
```

### Integration Tests

```rust
#[test]
fn test_cmd_in_macro() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // cmd can be used in macro sequences
    assert!(validator.validate_action(
        "(macro esc (cmd notify-send \"Hello\") 100 esc)"
    ));
}

#[test]
fn test_cmd_in_multi() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // cmd can be combined with other actions
    assert!(validator.validate_action(
        "(multi (cmd echo start) esc (cmd echo end))"
    ));
}

#[test]
fn test_cmd_in_tap_hold() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // cmd can be used in tap-hold
    assert!(validator.validate_action(
        "(tap-hold 200 200 (cmd echo tap) (cmd echo hold))"
    ));
}

#[test]
fn test_cmd_complex_real_world() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Real-world complex commands
    assert!(validator.validate_action(
        "(cmd bazel build -c opt //src/...)"
    ));
    
    assert!(validator.validate_action(
        "(cmd git status --short)"
    ));
    
    assert!(validator.validate_action(
        "(cmd powershell.exe -Command \"Get-Process | Select-Object Name, CPU\")"
    ));
    
    assert!(validator.validate_action(
        "(cmd wtype \"special characters: àáâãäå\")"
    ));
}

#[test]
fn test_cmd_with_variables() {
    let data = create_test_data_with_defvars();
    let validator = KanataValidator::new(&data);
    
    // Can use variables for parts of command
    assert!(validator.validate_action("(cmd echo $message)"));
}
```

### Edge Cases

```rust
#[test]
fn test_cmd_empty_string_arg() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Empty quoted string is valid
    assert!(validator.validate_action("(cmd echo \"\")"));
}

#[test]
fn test_cmd_very_long_command() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Long commands with many args
    assert!(validator.validate_action(
        "(cmd a b c d e f g h i j k l m n o p q r s t u v w x y z)"
    ));
}

#[test]
fn test_cmd_path_with_spaces() {
    let data = create_test_data();
    let validator = KanataValidator::new(&data);
    
    // Paths with spaces must be quoted
    assert!(validator.validate_action(
        "(cmd \"/path with spaces/binary\" arg)"
    ));
}
```

## Implementation Phases

### Phase 1: Core Implementation (1 hour)
1. Add `ParamType::String` to enum
2. Add `cmd`, `cmd-log`, `cmd-output-keys` to `KANATA_ACTIONS`
3. Update validation logic:
   - Variadic string handling for `cmd` and `cmd-output-keys`
   - Fixed 2-param with log level validation for `cmd-log`

### Phase 2: UI Integration (30 min)
1. Update parameter type display in info panel
2. Add `ParamType::String` → "string" mapping

### Phase 3: Testing (1 hour)
1. Add all unit tests
2. Add integration tests
3. Test edge cases

## Current Status
✅ **IMPLEMENTED**

- [x] ParamType::String added
- [x] cmd, cmd-log, cmd-output-keys actions defined
- [x] Validation for variadic strings
- [x] Validation for log levels (debug|info|warn|error|none)
- [x] All tests implemented and passing
