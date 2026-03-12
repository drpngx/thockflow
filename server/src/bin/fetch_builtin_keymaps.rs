//! Fetch and embed built-in keymap files from ZMK repository
//!
//! This binary finds all .keymap files in the ZMK app/boards directory
//! and generates a Rust module with them embedded as static strings.
//!
//! Usage:
//!   # With runfiles (Bazel build):
//!   bazel run //server:fetch_builtin_keymaps
//!
//!   # With explicit path:
//!   bazel run //server:fetch_builtin_keymaps -- <zmk_boards_dir> [output_path]

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a built-in keymap
#[derive(Debug, Clone)]
struct BuiltinKeymap {
    name: String,
    display_name: String,
    board_path: String,
    content: String,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let (input_path, output_path) = if args.len() >= 2 {
        // Explicit path provided - use it directly
        let input = std::path::PathBuf::from(&args[1]);
        let out = if args.len() >= 3 {
            Some(std::path::PathBuf::from(&args[2]))
        } else if let Some(ws_root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
            let mut p = std::path::PathBuf::from(ws_root);
            p.push("src/keymap/builtin_keymaps.rs");
            Some(p)
        } else {
            Some(std::path::PathBuf::from("src/keymap/builtin_keymaps.rs"))
        };
        (input, out)
    } else {
        // No arguments - use runfiles to find the ZMK boards directory
        #[cfg(feature = "runfiles")]
        {
            let r = runfiles::Runfiles::create()?;
            let input = runfiles::rlocation!(
                r,
                "zmk/app/boards"
            )
            .context("Failed to find ZMK boards directory in runfiles")?;
            let out = if let Some(ws_root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
                let mut p = std::path::PathBuf::from(ws_root);
                p.push("src/keymap/builtin_keymaps.rs");
                Some(p)
            } else {
                Some(std::path::PathBuf::from("src/keymap/builtin_keymaps.rs"))
            };
            (input, out)
        }
        #[cfg(not(feature = "runfiles"))]
        {
            eprintln!("Usage: {} <zmk_boards_dir> [output_path]", args[0]);
            eprintln!("");
            eprintln!("Finds and embeds keymap files from the ZMK repository.");
            eprintln!("");
            eprintln!("This binary requires the 'runfiles' feature to be enabled when built with Bazel,");
            eprintln!("or you can manually provide the boards directory.");
            eprintln!("");
            eprintln!("Example:");
            eprintln!("  {} /path/to/zmk/app/boards", args[0]);
            std::process::exit(1);
        }
    };

    let mut keymaps = Vec::new();

    // Find all .keymap files
    if input_path.is_dir() {
        find_keymap_files(&input_path, &mut keymaps)?;
    } else {
        anyhow::bail!("Input path is not a directory: {:?}", input_path);
    }

    // Sort by name for consistent output
    keymaps.sort_by(|a, b| a.name.cmp(&b.name));

    // Generate output
    let output = generate_rust_code(&keymaps);

    if let Some(out_path) = output_path {
        fs::write(&out_path, output)?;
        println!("Generated {} keymaps to {:?}", keymaps.len(), out_path);
    } else {
        println!("{}", output);
    }

    Ok(())
}

fn find_keymap_files(dir: &Path, keymaps: &mut Vec<BuiltinKeymap>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            find_keymap_files(&path, keymaps)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("keymap") {
            if let Some(keymap) = process_keymap_file(&path)? {
                keymaps.push(keymap);
            }
        }
    }
    Ok(())
}

fn process_keymap_file(path: &Path) -> Result<Option<BuiltinKeymap>> {
    let content = fs::read_to_string(path)?;
    
    // Skip files that are just includes or empty
    if content.trim().is_empty() || content.trim().starts_with("#include") && content.lines().count() < 3 {
        return Ok(None);
    }
    
    // Skip native_sim (simulator) keymaps
    let path_str = path.to_string_lossy();
    if path_str.contains("native_sim") {
        return Ok(None);
    }

    // Extract board name from path
    // Path format: .../boards/<manufacturer>/<board>/<board>.keymap
    // or: .../boards/shields/<shield>/<shield>.keymap
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    
    // Find the position of "boards" in the path
    if let Some(boards_idx) = components.iter().position(|c| c == "boards") {
        // Get the relative path from boards/
        let rel_components = &components[boards_idx + 1..];
        let board_path = rel_components.join("/");
        
        // Extract name (filename without extension)
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Create display name from path components
        // shields/sofle/sofle.keymap -> "sofle"
        // keebio/bdn9/bdn9.keymap -> "bdn9"
        let display_name = if rel_components.len() >= 2 {
            // Use the directory name (which is usually the board name)
            rel_components[rel_components.len() - 2].clone()
        } else {
            name.clone()
        };

        Ok(Some(BuiltinKeymap {
            name,
            display_name,
            board_path,
            content,
        }))
    } else {
        // Fallback if "boards" not in path
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let board_path = path.to_string_lossy().to_string();
        
        Ok(Some(BuiltinKeymap {
            name: name.clone(),
            display_name: name,
            board_path,
            content,
        }))
    }
}

fn escape_rust_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn generate_rust_code(keymaps: &[BuiltinKeymap]) -> String {
    let mut output = String::new();
    output.push_str("// Generated by server/src/bin/fetch_builtin_keymaps.rs. DO NOT EDIT.\n");
    output.push_str("// Source: https://github.com/zmkfirmware/zmk\n");
    output.push_str("// To regenerate: bazel run //server:fetch_builtin_keymaps\n\n");
    
    output.push_str("pub struct BuiltinKeymap {\n");
    output.push_str("    pub name: &'static str,\n");
    output.push_str("    pub display_name: &'static str,\n");
    output.push_str("    pub board_path: &'static str,\n");
    output.push_str("    pub content: &'static str,\n");
    output.push_str("}\n\n");
    
    output.push_str("pub const BUILTIN_KEYMAPS: &[BuiltinKeymap] = &[\n");
    
    for keymap in keymaps {
        output.push_str("    BuiltinKeymap {\n");
        output.push_str(&format!("        name: \"{}\",\n", keymap.name));
        output.push_str(&format!("        display_name: \"{}\",\n", keymap.display_name));
        output.push_str(&format!("        board_path: \"{}\",\n", keymap.board_path));
        output.push_str(&format!("        content: \"{}\",\n", escape_rust_string(&keymap.content)));
        output.push_str("    },\n");
    }
    
    output.push_str("];\n");
    output
}
