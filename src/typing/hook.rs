use yew::prelude::*;
use super::quotes;
use super::matching::{align_incremental, EditOp};

pub struct TypingGameReturn {
    pub current_quote: String,
    pub user_input: String,
    #[allow(dead_code)]
    pub current_position: usize,
    #[allow(dead_code)]
    pub started: bool,
    pub finished: bool,
    pub scroll_offset: usize, // Expose scroll_offset
    pub on_keydown: Callback<web_sys::KeyboardEvent>,
    pub div_ref: NodeRef,
    #[allow(dead_code)]
    pub reset: Callback<()>,
    pub set_scroll_offset: Callback<usize>, // Expose setter
    
    // Stats & Data
    pub error_count: usize,
    #[allow(dead_code)]
    pub total_typed_chars: usize,
    pub keystroke_times: Vec<f64>,
    pub start_time: Option<f64>,
    #[allow(dead_code)]
    pub end_time: Option<f64>,
    pub error_positions: Vec<usize>,
    pub key_log: String,
    
    // Pre-calculated stats
    pub wpm: f64,
    pub cpm: f64,
    pub accuracy: f64,
    pub elapsed_seconds: f64,
    pub total_chars: usize,
    pub total_words: usize,
}

#[hook]
pub fn use_typing_game() -> TypingGameReturn {
    let quote_context = use_context::<Option<crate::QuoteContext>>().flatten();
    
    let current_quote = use_state(|| {
        if let Some(ctx) = quote_context {
            return quotes::QUOTES[ctx.index % quotes::QUOTES.len()].to_string();
        }
        let idx = (js_sys::Math::random() * quotes::QUOTES.len() as f64) as usize;
        quotes::QUOTES[idx].to_string()
    });
    let user_input = use_state(|| String::new());
    let current_position = use_state(|| 0usize); // Track position in quote
    let started = use_state(|| false);
    let finished = use_state(|| false);
    let scroll_offset = use_state(|| 0usize); // Added scroll_offset
    let start_time = use_state(|| None::<f64>);
    let end_time = use_state(|| None::<f64>);
    let error_count = use_state(|| 0usize);
    
    let keystroke_times = use_state(|| Vec::<f64>::new()); // Timestamp for each keystroke
    let error_positions = use_state(|| Vec::<usize>::new()); // Positions where errors occurred
    let total_typed_chars = use_state(|| 0usize); // Total characters typed (including errors)
    let key_log = use_state(|| String::new()); // Log of all keys pressed
    let div_ref = use_node_ref();

    // Auto-focus on mount
    {
        let div_ref = div_ref.clone();
        use_effect(move || {
            if let Some(element) = div_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
            || ()
        });
    }

    let reset = {
        let user_input = user_input.clone();
        let current_position = current_position.clone();
        let started = started.clone();
        let finished = finished.clone();
        let scroll_offset = scroll_offset.clone(); // Capture scroll_offset
        let start_time = start_time.clone();
        let end_time = end_time.clone();
        let current_quote = current_quote.clone();
        let error_count = error_count.clone();
        let keystroke_times = keystroke_times.clone();
        let error_positions = error_positions.clone();
        let total_typed_chars = total_typed_chars.clone();
        let key_log = key_log.clone();
        let div_ref = div_ref.clone();

        Callback::from(move |_| {
            let idx = (js_sys::Math::random() * quotes::QUOTES.len() as f64) as usize;
            current_quote.set(quotes::QUOTES[idx].to_string());
            scroll_offset.set(0); // Reset scroll_offset
            user_input.set(String::new());
            current_position.set(0);
            started.set(false);
            finished.set(false);
            start_time.set(None);
            end_time.set(None);
            error_count.set(0);
            keystroke_times.set(Vec::new());
            error_positions.set(Vec::new());
            total_typed_chars.set(0);
            key_log.set(String::new());

            // Re-focus after reset
            if let Some(element) = div_ref.cast::<web_sys::HtmlElement>() {
                let _ = element.focus();
            }
        })
    };

    let on_keydown = {
        let user_input = user_input.clone();
        let current_position = current_position.clone();
        let started = started.clone();
        let finished = finished.clone();
        let start_time = start_time.clone();
        let end_time = end_time.clone();
        let current_quote = current_quote.clone();
        let error_count = error_count.clone();
        let keystroke_times = keystroke_times.clone();
        let error_positions = error_positions.clone();
        let total_typed_chars = total_typed_chars.clone();
        let key_log = key_log.clone();
        let reset = reset.clone();

        Callback::from(move |e: web_sys::KeyboardEvent| {
            let key = e.key();

            // Tab always starts new quote
            if key == "Tab" {
                e.prevent_default();
                reset.emit(());
                return;
            }

            // Escape: finish early if started, otherwise reset
            if key == "Escape" {
                e.prevent_default();
                if *started {
                    finished.set(true);
                    end_time.set(Some(js_sys::Date::now()));
                    started.set(false);
                } else {
                    reset.emit(());
                }
                return;
            }

            // If finished, don't process other keys
            if *finished {
                return;
            }

            // Handle backspace
            if key == "Backspace" {
                e.prevent_default();
                
                // Log backspace
                let mut log = (*key_log).clone();
                if e.ctrl_key() {
                    log.push_str("[Ctrl+BS]");
                } else {
                    log.push_str("[BS]");
                }
                key_log.set(log);

                if *current_position > 0 {
                    let mut current = (*user_input).clone();

                    if e.ctrl_key() {
                        // Ctrl+Backspace: Delete word
                        // 1. Remove trailing whitespace
                        while let Some(c) = current.chars().last() {
                            if c.is_whitespace() {
                                current.pop();
                            } else {
                                break;
                            }
                        }
                        // 2. Remove trailing non-whitespace
                        while let Some(c) = current.chars().last() {
                            if !c.is_whitespace() {
                                current.pop();
                            } else {
                                break;
                            }
                        }
                    } else {
                        current.pop();
                    }

                    let new_len = current.chars().count();
                    user_input.set(current);
                    current_position.set(new_len);

                    // Sync keystroke times
                    let mut times = (*keystroke_times).clone();
                    if times.len() > new_len {
                        times.truncate(new_len);
                        keystroke_times.set(times);
                    }
                }
                return;
            }

            // Only process single character keys
            if key.len() != 1 {
                return;
            }

            e.prevent_default();

            // Log key
            let mut log = (*key_log).clone();
            log.push_str(&key);
            key_log.set(log);

            let now = js_sys::Date::now();

            if !*started {
                started.set(true);
                start_time.set(Some(now));
            }

            // Increment total typed chars
            total_typed_chars.set(*total_typed_chars + 1);

            // Record keystroke time
            let mut times = (*keystroke_times).clone();
            times.push(now);
            keystroke_times.set(times.clone());

            let mut current = (*user_input).clone();
            current.push_str(&key);

            user_input.set(current.clone());

            // Run alignment to check for errors and completion
            // We use the dynamic alignment to determine if the LAST typed char was an error.
            // This handles skips correctly (skipping text doesn't make subsequent correct typing an error).
            let alignment = align_incremental(&current_quote, &current);

            // Find the index of the operation corresponding to the last input character
            let mut last_input_idx = None;
            for (i, (_, _, input_char)) in alignment.iter().enumerate().rev() {
                if input_char.is_some() {
                    last_input_idx = Some(i);
                    break;
                }
            }

            if let Some(idx) = last_input_idx {
                let (op, _, _) = alignment[idx];
                let is_error = match op {
                    EditOp::Match => {
                        // Even if it's a match, it's an error if we skipped something to get here
                        // Check if the immediately preceding op was a Skip (Substitute with None input)
                        if idx > 0 {
                            let (prev_op, _, prev_input) = alignment[idx - 1];
                            // If previous op was a Substitute and it didn't consume input, it's a Skip
                            matches!(prev_op, EditOp::Substitute) && prev_input.is_none()
                        } else {
                            false
                        }
                    },
                    EditOp::Substitute => true,
                    EditOp::Insert => true,
                };

                if is_error {
                    error_count.set(*error_count + 1);
                    // Record error position (index in keystroke_times)
                    let mut errors = (*error_positions).clone();
                    errors.push(times.len() - 1); // Index of the key just added
                    error_positions.set(errors);
                }
            }

            // Calculate new position (just length of input)
            let new_position = current.chars().count();
            current_position.set(new_position);

            // Check if finished - based on alignment consuming all quote characters
            let consumed_quote_chars = alignment.iter()
                .filter(|(op, _, _)| *op != EditOp::Insert)
                .count();

            // Quote is finished if we consumed all chars (matches + skips)
            // But we also want to ensure the user is at the end of their typing (implied)
            let quote_len = current_quote.chars().count();
            if consumed_quote_chars >= quote_len {
                 finished.set(true);
                 end_time.set(Some(js_sys::Date::now()));
                 started.set(false);
            }
        })
    };

    // Calculate statistics
    let total_chars = current_quote.chars().count();
    let total_words = current_quote.split_whitespace().count();

    let (wpm, cpm, accuracy, elapsed_seconds) = if *finished {
        if let (Some(start), Some(end)) = (*start_time, *end_time) {
            let elapsed_ms = end - start;
            let elapsed_sec = elapsed_ms / 1000.0;
            let elapsed_min = elapsed_sec / 60.0;

            // For partial completion, base stats on what was typed
            let typed_len = user_input.chars().count();
            let (calc_len, _dist_quote) = if typed_len < total_chars {
                // Partial: compare input against prefix of quote
                (typed_len, current_quote.chars().take(typed_len).collect::<String>())
            } else {
                // Full: compare input against full quote
                (total_chars, (*current_quote).clone())
            };

            let cpm = if elapsed_min > 0.0 { calc_len as f64 / elapsed_min } else { 0.0 };
            let wpm = if elapsed_min > 0.0 { (calc_len as f64 / 5.0) / elapsed_min } else { 0.0 };

            let accuracy = if *total_typed_chars > 0 {
                (1.0 - (*error_count as f64 / *total_typed_chars as f64)) * 100.0
            } else {
                100.0
            };

            (wpm, cpm, accuracy.max(0.0), elapsed_sec)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    let set_scroll_offset = {
        let scroll_offset = scroll_offset.clone();
        Callback::from(move |offset| scroll_offset.set(offset))
    };

    TypingGameReturn {
        current_quote: (*current_quote).clone(),
        user_input: (*user_input).clone(),
        current_position: *current_position,
        started: *started,
        finished: *finished,
        scroll_offset: *scroll_offset,
        on_keydown,
        div_ref,
        reset,
        set_scroll_offset,
        error_count: *error_count,
        total_typed_chars: *total_typed_chars,
        keystroke_times: (*keystroke_times).clone(),
        start_time: *start_time,
        end_time: *end_time,
        error_positions: (*error_positions).clone(),
        key_log: (*key_log).clone(),
        wpm,
        cpm,
        accuracy,
        elapsed_seconds,
        total_chars,
        total_words,
    }
}
