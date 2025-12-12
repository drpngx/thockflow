use yew::{function_component, html, Html, use_node_ref, use_effect, NodeRef};
use web_sys::Element;

mod matching;
mod quotes;

mod hook;
mod results;

use matching::{align_incremental, EditOp};

#[function_component]
pub fn TypingHome() -> Html {
    let game = hook::use_typing_game();
    
    // Refs for smooth cursor
    let active_char_ref = use_node_ref();
    let cursor_ref = use_node_ref();
    let marker_ref = use_node_ref();

    // Effect to update cursor position
    {
        let active_char_ref = active_char_ref.clone();
        let cursor_ref = cursor_ref.clone();
        let marker_ref = marker_ref.clone();
        use_effect(move || {
            if let (Some(active_char_el), Some(cursor_el), Some(marker_el)) = (
                active_char_ref.cast::<Element>(),
                cursor_ref.cast::<Element>(),
                marker_ref.cast::<Element>(),
            ) {
                let char_rect = active_char_el.get_bounding_client_rect();
                let marker_rect = marker_el.get_bounding_client_rect();
                
                let left = char_rect.left() - marker_rect.left();
                let top = char_rect.top() - marker_rect.top();
                let height = char_rect.height();
                
                // Construct the full style string.
                let style_str = format!("left: {}px; top: {}px; height: {}px; opacity: 1;", left, top, height);
                let _ = cursor_el.set_attribute("style", &style_str);
            } else if let Some(cursor_el) = cursor_ref.cast::<Element>() {
                 // If active ref is missing (e.g. init or glitch), hide cursor.
                 let _ = cursor_el.set_attribute("style", "opacity: 0;");
            }
            || ()
        });
    }

    // Render logic for the active game view
    // We compute this even if finished to avoid conditional logic complexity inside the component body,
    // although strictly we only need it if !game.finished.
    // Optimization: check !game.finished.
    
    let game_view = if !game.finished {
        let current_quote = &game.current_quote;
        let user_input = &game.user_input;

        // Split quote into words and group into lines
        let words: Vec<&str> = current_quote.split_whitespace().collect();
        let chars_per_line = 55; // Approximate chars that fit in 70vw at text-4xl

        // Group words into lines based on character count
        let mut lines: Vec<Vec<&str>> = Vec::new();
        let mut current_line_words: Vec<&str> = Vec::new();
        let mut current_line_len = 0;

        for word in &words {
            let word_len = word.chars().count() + 1; // +1 for space
            if current_line_len + word_len > chars_per_line && !current_line_words.is_empty() {
                lines.push(current_line_words);
                current_line_words = Vec::new();
                current_line_len = 0;
            }
            current_line_words.push(word);
            current_line_len += word_len;
        }
        if !current_line_words.is_empty() {
            lines.push(current_line_words);
        }

        // Get alignment to find cursor position
        let alignment = align_incremental(current_quote, user_input);
        let consumed_quote_chars = alignment.iter()
            .filter(|(op, _, _)| *op != EditOp::Insert)
            .count();

        // Find which line the cursor is on
        let mut char_count = 0;
        let mut cursor_line = 0;
        for (i, line_words) in lines.iter().enumerate() {
            let line_char_count: usize = line_words.iter().map(|w| w.chars().count() + 1).sum();
            if char_count + line_char_count > consumed_quote_chars {
                cursor_line = i;
                break;
            }
            char_count += line_char_count;
            cursor_line = i + 1;
        }

        // Scroll offset: only advances forward, keeps cursor on line 2 (middle)
        let new_scroll = if cursor_line <= 1 { 0 } else { cursor_line - 1 };
        let current_scroll = game.scroll_offset;
        
        // Side effect: update scroll offset if needed. 
        // Note: calling state setter during render is generally bad practice in React/Yew, 
        // but here it simulates "derived state with side effect". 
        // A better approach would be to calculate visible lines based on cursor position directly without stored state,
        // but the original code used stored state to prevent scrolling backward or jumping?
        // Original code: "Scroll offset: only advances forward".
        // So we need to respect the stored state.
        
        let effective_scroll = if new_scroll > current_scroll {
            game.set_scroll_offset.emit(new_scroll);
            new_scroll
        } else {
            current_scroll
        };
        
        let scroll = effective_scroll;

        // Get the 3 lines to display
        let visible_lines: Vec<&Vec<&str>> = lines.iter().skip(scroll).take(3).collect();

        // Build the text for visible lines with alignment coloring
        let mut visible_start_char = 0;
        for line_words in lines.iter().take(scroll) {
            visible_start_char += line_words.iter().map(|w| w.chars().count() + 1).sum::<usize>();
        }

        let mut insertions_before: std::collections::HashMap<usize, Vec<char>> = std::collections::HashMap::new();
        let mut char_status: std::collections::HashMap<usize, (bool, Option<char>)> = std::collections::HashMap::new();

        let mut quote_pos = 0;
        for (op, _, input_char) in alignment.iter() {
            match op {
                EditOp::Insert => {
                    insertions_before.entry(quote_pos).or_insert_with(Vec::new).push(input_char.unwrap());
                }
                EditOp::Match => {
                    char_status.insert(quote_pos, (false, None));
                    quote_pos += 1;
                }
                EditOp::Substitute => {
                    char_status.insert(quote_pos, (true, *input_char));
                    quote_pos += 1;
                }
            }
        }

        let mut rendered_lines: Vec<Html> = Vec::new();
        let mut pos = visible_start_char;

        for line_words in visible_lines.iter() {
            let mut line_elements: Vec<Html> = Vec::new();

            for (word_idx, word) in line_words.iter().enumerate() {
                for ch in word.chars() {
                    if let Some(inserts) = insertions_before.get(&pos) {
                        for &ins_char in inserts {
                            let display_char = if ins_char == ' ' { '\u{2423}' } else { ins_char };
                            line_elements.push(html! {
                                <span class="text-red-500 dark:text-red-400 bg-red-900/30 line-through">{display_char}</span>
                            });
                        }
                    }

                    let (is_error, typed_char) = char_status.get(&pos).cloned().unwrap_or((false, None));
                    let show_cursor = pos == consumed_quote_chars;
                    
                    // Attach active_char_ref if this is the cursor position
                    let node_ref = if show_cursor { active_char_ref.clone() } else { NodeRef::default() };

                    let class = if pos < consumed_quote_chars {
                        if is_error {
                            "text-red-500 dark:text-red-400 bg-red-900/30"
                        } else {
                            "text-white dark:text-white"
                        }
                    } else {
                        "text-gray-500 dark:text-gray-500"
                    };

                    line_elements.push(html! {
                        <span class="relative inline">
                            <span ref={node_ref} class={class}>{ch}</span>
                            {if let Some(typed) = typed_char {
                                html! { <span class="absolute text-xs text-red-300" style="top: 100%; left: 0; line-height: 1;">{typed}</span> }
                            } else {
                                html! {}
                            }}
                        </span>
                    });
                    pos += 1;
                }
                
                if word_idx < line_words.len() - 1 {
                    if let Some(inserts) = insertions_before.get(&pos) {
                        for &ins_char in inserts {
                            let display_char = if ins_char == ' ' { '\u{2423}' } else { ins_char };
                            line_elements.push(html! {
                                <span class="text-red-500 dark:text-red-400 bg-red-900/30 line-through">{display_char}</span>
                            });
                        }
                    }

                    let (is_error, typed_char) = char_status.get(&pos).cloned().unwrap_or((false, None));
                    let show_cursor = pos == consumed_quote_chars;
                    let node_ref = if show_cursor { active_char_ref.clone() } else { NodeRef::default() };

                    let class = if pos < consumed_quote_chars {
                        if is_error {
                            "text-red-500 dark:text-red-400 bg-red-900/30"
                        } else {
                            "text-white dark:text-white"
                        }
                    } else {
                        "text-gray-500 dark:text-gray-500"
                    };

                    line_elements.push(html! {
                        <span class="relative inline">
                            <span ref={node_ref} class={class}>{" "}</span>
                            {if let Some(typed) = typed_char {
                                html! { <span class="absolute text-xs text-red-300" style="top: 100%; left: 0; line-height: 1;">{typed}</span> }
                            } else {
                                html! {}
                            }}
                        </span>
                    });
                    pos += 1;
                }
            }
            if pos < current_quote.chars().count() {
                // Handle cursor at end of line (split by wrapping)
                if pos == consumed_quote_chars {
                    line_elements.push(html! { <span ref={active_char_ref.clone()} class="inline-block w-0 h-8 align-middle"></span> });
                }
                pos += 1; 
            }

            rendered_lines.push(html! {
                <div class="whitespace-nowrap overflow-hidden">
                    {line_elements.into_iter().collect::<Html>()}
                </div>
            });
        }

        let rendered_text = rendered_lines.into_iter().collect::<Html>();

        let total_chars = current_quote.chars().count();
        let progress_pct = if total_chars > 0 {
            (consumed_quote_chars as f64 / total_chars as f64) * 100.0
        } else {
            0.0
        };

        html! {
            <>
            <div class="w-full h-1.5 bg-gray-200 rounded-full mb-6 dark:bg-gray-700">
                <div class="h-1.5 bg-blue-500 rounded-full dark:bg-blue-400 transition-all duration-200 ease-out" style={format!("width: {:.1}%", progress_pct)}></div>
            </div>
            <div class="p-6 bg-gray-100 dark:bg-gray-800 rounded-lg relative">
                 // Position Marker
                 <div ref={marker_ref} class="absolute top-0 left-0 w-0 h-0 pointer-events-none"></div>
                 // Smooth Cursor
                 <div ref={cursor_ref} class="absolute w-0.5 bg-yellow-400 transition-all duration-100 ease-out z-10 pointer-events-none" 
                      style="left: 0; top: 0; height: 1.5em; opacity: 1;"></div>
                
                <div class="text-4xl font-mono select-none relative z-0" style="line-height: 1.8;">
                    {rendered_text}
                </div>
            </div>
            </>
        }
    } else {
        html! {}
    };

    html! {
        <div ref={game.div_ref} class="w-full px-4 focus:outline-none" tabindex="0" onkeydown={game.on_keydown} style="max-width: 70vw; margin: 0 auto;">
            <h2 class="text-3xl font-bold mb-4 text-center">{"ThockFlow"}</h2>

            if !game.finished {
                {game_view}
            } else {
                <results::TypingResults
                    wpm={game.wpm}
                    cpm={game.cpm}
                    accuracy={game.accuracy}
                    elapsed_seconds={game.elapsed_seconds}
                    total_chars={game.total_chars}
                    total_words={game.total_words}
                    error_count={game.error_count}
                    keystroke_times={game.keystroke_times}
                    start_time={game.start_time}
                    error_positions={game.error_positions}
                    current_quote={game.current_quote}
                    user_input={game.user_input}
                    key_log={game.key_log}
                />
            }
        </div>
    }
}
