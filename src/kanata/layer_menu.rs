//! Layer menu implementation for Kanata tab
//! 
//! Provides layer manipulation operations similar to the Vial tab:
//! - Move Up/Down: Reorder layers
//! - Rename: Change layer name
//! - Duplicate: Create a copy of the layer
//! - Delete: Remove the layer (with confirmation)
//! - Reset All to None: Set all keys to "_"
//! - Trans → None: Convert "_" to "none"
//! - None → Trans: Convert "none" to "_"
//! - Quick Assignment: Start quick key assignment mode

use std::collections::HashMap;
use std::rc::Rc;
use yew::prelude::*;

/// Hint target types for jump mode navigation
#[derive(Clone, PartialEq, Debug)]
pub enum HintTarget {
    Layer(usize),
    LayerMenu(usize),
    Menu(usize, usize),  // (layer_index, menu_item_index)
    Key(usize),
}

/// State for layer menu operations
#[derive(Clone, PartialEq)]
pub struct LayerMenuState {
    pub menu_open_index: Option<usize>,
    pub focus_index: usize,
    pub quick_assign_index: Option<usize>,
}

impl Default for LayerMenuState {
    fn default() -> Self {
        Self {
            menu_open_index: None,
            focus_index: 0,
            quick_assign_index: None,
        }
    }
}

/// Layer operation functions (passed to components)
pub struct LayerOperations {
    pub move_layer: Rc<dyn Fn(usize, bool)>,  // (idx, up)
    pub rename_layer: Rc<dyn Fn(usize)>,
    pub duplicate_layer: Rc<dyn Fn(usize)>,
    pub delete_layer: Rc<dyn Fn(usize)>,
    pub reset_layer: Rc<dyn Fn(usize)>,
    pub trans_to_none: Rc<dyn Fn(usize)>,
    pub none_to_trans: Rc<dyn Fn(usize)>,
    pub start_quick_assign: Rc<dyn Fn()>,
}



/// Build the hint map for jump mode
pub fn build_hint_map(
    num_keys: usize,
    num_layers: usize,
    menu_open_index: Option<usize>,
) -> (HashMap<String, HintTarget>, Vec<String>, Vec<String>) {
    let hint_chars = "asdfghjklqwertyuiopzxcvbnm";
    let mut hint_map: HashMap<String, HintTarget> = HashMap::new();
    let mut key_hints = vec![String::new(); num_keys];
    let mut layer_hints = vec![String::new(); num_layers];
    
    let mut all_targets = Vec::new();
    
    // Add layer targets
    for i in 0..num_layers {
        all_targets.push(HintTarget::Layer(i));
        all_targets.push(HintTarget::LayerMenu(i));
        
        // Add menu items if menu is open for this layer
        if let Some(lmi) = menu_open_index {
            if lmi == i {
                for j in 0..9 {
                    all_targets.push(HintTarget::Menu(i, j));
                }
            }
        }
    }
    
    // Add key targets
    for i in 0..num_keys {
        all_targets.push(HintTarget::Key(i));
    }
    
    // Assign hints to targets
    for (i, target) in all_targets.into_iter().enumerate() {
        if i < hint_chars.len() * hint_chars.len() {
            let h = format!(
                "{}{}",
                hint_chars.chars().nth(i / hint_chars.len()).unwrap(),
                hint_chars.chars().nth(i % hint_chars.len()).unwrap()
            );
            
            match target {
                HintTarget::Layer(idx) => layer_hints[idx] = h.clone(),
                HintTarget::Key(idx) => key_hints[idx] = h.clone(),
                _ => {}
            }
            hint_map.insert(h, target);
        }
    }
    
    (hint_map, key_hints, layer_hints)
}

/// Handle keyboard navigation for the menu
/// Returns true if the event was handled
pub fn handle_menu_keydown(
    e: &KeyboardEvent,
    menu_state: &UseStateHandle<LayerMenuState>,
    layer_ops: &LayerOperations,
) -> bool {
    if let Some(lmi) = menu_state.menu_open_index {
        match e.key().as_str() {
            "ArrowDown" => {
                let new_focus = (menu_state.focus_index + 1) % 9;
                menu_state.set(LayerMenuState {
                    menu_open_index: Some(lmi),
                    focus_index: new_focus,
                    quick_assign_index: menu_state.quick_assign_index,
                });
                return true;  // Handled
            }
            "ArrowUp" => {
                let new_focus = (menu_state.focus_index + 8) % 9;
                menu_state.set(LayerMenuState {
                    menu_open_index: Some(lmi),
                    focus_index: new_focus,
                    quick_assign_index: menu_state.quick_assign_index,
                });
                return true;  // Handled
            }
            "Enter" => {
                match menu_state.focus_index {
                    0 => (layer_ops.move_layer)(lmi, true),
                    1 => (layer_ops.move_layer)(lmi, false),
                    2 => (layer_ops.rename_layer)(lmi),
                    3 => (layer_ops.duplicate_layer)(lmi),
                    4 => (layer_ops.delete_layer)(lmi),
                    5 => (layer_ops.reset_layer)(lmi),
                    6 => (layer_ops.trans_to_none)(lmi),
                    7 => (layer_ops.none_to_trans)(lmi),
                    8 => (layer_ops.start_quick_assign)(),
                    _ => {}
                }
                return true;  // Handled
            }
            "Escape" => {
                menu_state.set(LayerMenuState::default());
                return true;  // Handled
            }
            _ => {}
        }
    }
    false  // Not handled
}

/// Execute a menu action by index
pub fn execute_menu_action(
    layer_index: usize,
    menu_index: usize,
    layer_ops: &LayerOperations,
) {
    match menu_index {
        0 => (layer_ops.move_layer)(layer_index, true),
        1 => (layer_ops.move_layer)(layer_index, false),
        2 => (layer_ops.rename_layer)(layer_index),
        3 => (layer_ops.duplicate_layer)(layer_index),
        4 => (layer_ops.delete_layer)(layer_index),
        5 => (layer_ops.reset_layer)(layer_index),
        6 => (layer_ops.trans_to_none)(layer_index),
        7 => (layer_ops.none_to_trans)(layer_index),
        8 => (layer_ops.start_quick_assign)(),
        _ => {}
    }
}
