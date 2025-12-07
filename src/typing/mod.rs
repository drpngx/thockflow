use std::borrow::Borrow;

use yew::{
    function_component, html, mdx_style, use_callback, use_effect, use_node_ref, use_state, Callback, Children, Html,
    Properties,
};

mod quotes;
mod matching;

use matching::{align_incremental, EditOp};

macro_rules! typing_style {
    () => {
        mdx_style!(
            h1: MyH1,
            h2: MyH2,
            h3: MyH3,
            blockquote: MyBlockquote,
            pre: MyBlockquote,
            p: MyP,
            li: MyLi,
            ul: MyUl,
            code: MyCode,
        );
    };
}
pub(crate) use typing_style;

#[derive(PartialEq, Properties)]
pub struct ChildProps {
    #[prop_or_default]
    children: Children,
}

const HEADER_LINK_LEN: usize = 20;

#[function_component]
fn MyH1(c: &ChildProps) -> Html {
    let mut tag = String::new();
    for c in c.children.iter() {
        match c {
            yew::virtual_dom::VNode::VText(t) => {
                tag += &t.text.to_string();
            }
            _ => (),
        };
    }
    tag = tag.replace(" ", "-").to_lowercase();
    tag.truncate(HEADER_LINK_LEN);
    html! {
      <h1 id={tag.clone()} class="text-4xl pt-10 pb-6">
        <a class="text-inherit" href={format!("#{tag}")}>
          {c.children.clone()}
        </a>
      </h1>
    }
}

#[function_component]
fn MyH2(c: &ChildProps) -> Html {
    let mut tag = String::new();
    for c in c.children.iter() {
        match c {
            yew::virtual_dom::VNode::VText(t) => {
                tag += &t.text.to_string();
            }
            _ => (),
        };
    }
    tag = tag.replace(" ", "-").to_lowercase();
    tag.truncate(HEADER_LINK_LEN);
    html! {
      <h2 id={tag.clone()} class="text-2xl pt-8 pb-4">
        <a class="text-inherit" href={format!("#{tag}")}>
          {c.children.clone()}
        </a>
      </h2>
    }
}

#[function_component]
fn MyH3(c: &ChildProps) -> Html {
    let tag = children_to_slug(c.children.iter());
    html! {
      <h3 id={tag.clone()} class="text-xl pt-6 pb-2">
        <a class="text-inherit" href={format!("#{tag}")}>
          {c.children.clone()}
        </a>
      </h3>
    }
}

fn children_to_slug(c: impl IntoIterator<Item = Html>) -> String {
    let mut out = children_to_string(c);
    out.truncate(HEADER_LINK_LEN);
    out
}

fn children_to_string<H: Borrow<Html>>(c: impl IntoIterator<Item = H>) -> String {
    let mut out = String::new();
    for c in c.into_iter() {
        match c.borrow() {
            yew::virtual_dom::VNode::VText(t) => {
                out += &t.text.to_string();
            }
            _ => (),
        };
    }
    out = out.replace(" ", "-").to_lowercase();

    out
}

#[function_component]
fn MyPre(c: &ChildProps) -> Html {
    html! {
      <pre class="overflow-auto m-4 p-6 bg-gray-300/5 rounded">
      {c.children.clone()}
      </pre>
    }
}

#[function_component]
fn MyBlockquote(c: &ChildProps) -> Html {
    html! {
      <blockquote class="text-black/70 dark:text-white/50 border-l-8 px-2 my-2 italic">
        {c.children.clone()}
      </blockquote>
    }
}

#[function_component]
fn MyP(c: &ChildProps) -> Html {
    html! {
      <p class="py-2 text-lg">
        {c.children.clone()}
      </p>
    }
}

#[function_component]
fn MyUl(c: &ChildProps) -> Html {
    html! {
      <div class="px-4">{c.children.clone()}</div>
    }
}

#[function_component]
fn MyLi(c: &ChildProps) -> Html {
    html! {
      <p class="py-1">{" - "}{c.children.clone()}</p>
    }
}

#[function_component]
fn MyCode(c: &ChildProps) -> Html {
    html! {
      <code class="bg-gray-300/40 dark:bg-gray-300/20 px-1 rounded">
        {c.children.clone()}
      </code>
    }
}

#[derive(Properties, PartialEq)]
struct HeyProps {
    name: String,
}
#[function_component]
fn HeyComponent(props: &HeyProps) -> Html {
    let count = use_state(|| 0);
    let s = count.to_string();

    html! {
        <>
        <p>{"said hey to "}{&props.name}{" "} <em onclick={ move |_| count.set(*count + 1)}>{s}</em>
        {" times"}</p>
        </>
    }
}

#[function_component]
fn Counter() -> Html {
    let count = use_state(|| 0);
    let click = use_callback(|_, [count]| count.set(**count + 1), [count.clone()]);

    html! {
        <button onclick={ click } class="bg-gray-300/30 rounded select-none p-2">
        {"Counter "}{*count}
        </button>
    }
}

#[function_component]
pub fn TypingHome() -> Html {
    let current_quote = use_state(|| {
        #[cfg(target_arch = "wasm32")]
        let idx = (js_sys::Math::random() * quotes::QUOTES.len() as f64) as usize;
        #[cfg(not(target_arch = "wasm32"))]
        let idx = 0;
        quotes::QUOTES[idx].to_string()
    });
    let user_input = use_state(|| String::new());
    let current_position = use_state(|| 0usize); // Track position in quote
    let started = use_state(|| false);
    let finished = use_state(|| false);
    let start_time = use_state(|| None::<f64>);
    let end_time = use_state(|| None::<f64>);
    let error_count = use_state(|| 0usize);
    let scroll_offset = use_state(|| 0usize); // Which line to start displaying from
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
        let start_time = start_time.clone();
        let end_time = end_time.clone();
        let current_quote = current_quote.clone();
        let error_count = error_count.clone();
        let scroll_offset = scroll_offset.clone();
        let keystroke_times = keystroke_times.clone();
        let error_positions = error_positions.clone();
        let total_typed_chars = total_typed_chars.clone();
        let key_log = key_log.clone();
        let div_ref = div_ref.clone();

        Callback::from(move |_| {
            let idx = (js_sys::Math::random() * quotes::QUOTES.len() as f64) as usize;
            current_quote.set(quotes::QUOTES[idx].to_string());
            scroll_offset.set(0);
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
            let (calc_len, dist_quote) = if typed_len < total_chars {
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

    // Calculate timeline data (WPM/CPM at each point)
    let timeline_data: Vec<(f64, f64, f64, bool)> = if *finished && keystroke_times.len() > 1 {
        let start = *start_time.as_ref().unwrap_or(&0.0);
        let mut data = Vec::new();

        for (i, &time) in keystroke_times.iter().enumerate() {
            let elapsed_min = (time - start) / 1000.0 / 60.0;
            if elapsed_min > 0.0 {
                let chars_so_far = i + 1;
                let cumulative_cpm = chars_so_far as f64 / elapsed_min;
                let cumulative_wpm = (chars_so_far as f64 / 5.0) / elapsed_min;

                // Instantaneous speed (based on last few keystrokes)
                let window = 5.min(i + 1);
                let instant_cpm = if i >= 1 {
                    let window_start_time = keystroke_times.get(i.saturating_sub(window)).unwrap_or(&start);
                    let window_elapsed = (time - window_start_time) / 1000.0 / 60.0;
                    if window_elapsed > 0.0 {
                        window as f64 / window_elapsed
                    } else {
                        cumulative_cpm
                    }
                } else {
                    cumulative_cpm
                };

                let is_error = error_positions.contains(&i);
                data.push((cumulative_wpm, instant_cpm, cumulative_cpm, is_error));
            }
        }
        data
    } else {
        Vec::new()
    };

    // Find max values for scaling the chart
    let max_wpm = timeline_data.iter().map(|(w, _, _, _)| *w).fold(0.0f64, f64::max);
    let max_cpm = timeline_data.iter().map(|(_, i, c, _)| i.max(*c)).fold(0.0f64, f64::max);
    let chart_max = (max_wpm.max(max_cpm / 5.0) * 1.1).max(1.0); // Add 10% headroom, min 1.0 to avoid div by zero

    // Calculate tick values for Y-axis
    let mut y_ticks: Vec<(f64, f64)> = Vec::new(); // (wpm_value, cpm_value)
    let display_max_wpm = (chart_max / 50.0).ceil() * 50.0; // Round up to nearest 50 WPM
    for i in 0..=((display_max_wpm / 50.0) as usize) {
        let wpm_val = i as f64 * 50.0;
        if wpm_val <= display_max_wpm {
            y_ticks.push((wpm_val, wpm_val * 5.0));
        }
    }

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
    let alignment = align_incremental(&current_quote, &user_input);
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
    // Once cursor reaches line 2 (index 1 in visible), we start scrolling
    let new_scroll = if cursor_line <= 1 { 0 } else { cursor_line - 1 };
    let current_scroll = *scroll_offset;
    if new_scroll > current_scroll {
        scroll_offset.set(new_scroll);
    }
    let scroll = *scroll_offset;

    // Get the 3 lines to display
    let visible_lines: Vec<&Vec<&str>> = lines.iter().skip(scroll).take(3).collect();

    // Build the text for visible lines with alignment coloring
    let mut visible_start_char = 0;
    for line_words in lines.iter().take(scroll) {
        visible_start_char += line_words.iter().map(|w| w.chars().count() + 1).sum::<usize>();
    }

    // Build a map: for each quote position, what insertions come before it, and what's the status
    // Also track insertions at the end (after all quote chars)
    let mut insertions_before: std::collections::HashMap<usize, Vec<char>> = std::collections::HashMap::new();
    let mut char_status: std::collections::HashMap<usize, (bool, Option<char>)> = std::collections::HashMap::new(); // (is_error, typed_char)

    let mut quote_pos = 0;
    for (op, _, input_char) in alignment.iter() {
        match op {
            EditOp::Insert => {
                // This insertion comes before quote_pos
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

    // Render the 3 visible lines
    let mut rendered_lines: Vec<Html> = Vec::new();
    let mut pos = visible_start_char;

    for line_words in visible_lines.iter() {
        let mut line_elements: Vec<Html> = Vec::new();

        for (word_idx, word) in line_words.iter().enumerate() {
            for ch in word.chars() {
                // First, render any insertions before this position
                if let Some(inserts) = insertions_before.get(&pos) {
                    for &ins_char in inserts {
                        // Use U+2423 (Open Box) to display inserted spaces visibly
                        let display_char = if ins_char == ' ' { '\u{2423}' } else { ins_char };
                        line_elements.push(html! {
                            <span class="text-red-500 dark:text-red-400 bg-red-900/30 line-through">{display_char}</span>
                        });
                    }
                }

                let (is_error, typed_char) = char_status.get(&pos).cloned().unwrap_or((false, None));
                let show_cursor = pos == consumed_quote_chars && !*finished;

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
                        {if show_cursor {
                            html! { <span class="absolute left-0 top-0 h-full w-0.5 bg-yellow-400 animate-pulse" style="margin-left: -1px;"></span> }
                        } else {
                            html! {}
                        }}
                        <span class={class}>{ch}</span>
                        {if let Some(typed) = typed_char {
                            html! { <span class="absolute text-xs text-red-300" style="top: 100%; left: 0; line-height: 1;">{typed}</span> }
                        } else {
                            html! {}
                        }}
                    </span>
                });
                pos += 1;
            }
            // Add space after word (except last word)
            if word_idx < line_words.len() - 1 {
                // First, render any insertions before this space
                if let Some(inserts) = insertions_before.get(&pos) {
                    for &ins_char in inserts {
                        // Use U+2423 (Open Box) to display inserted spaces visibly
                        let display_char = if ins_char == ' ' { '\u{2423}' } else { ins_char };
                        line_elements.push(html! {
                            <span class="text-red-500 dark:text-red-400 bg-red-900/30 line-through">{display_char}</span>
                        });
                    }
                }

                let (is_error, typed_char) = char_status.get(&pos).cloned().unwrap_or((false, None));
                let show_cursor = pos == consumed_quote_chars && !*finished;

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
                        {if show_cursor {
                            html! { <span class="absolute left-0 top-0 h-full w-0.5 bg-yellow-400 animate-pulse" style="margin-left: -1px;"></span> }
                        } else {
                            html! {}
                        }}
                        <span class={class}>{" "}</span>
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
        // Add space at end of line for word separation
        if pos < current_quote.chars().count() {
            pos += 1; // Account for space between lines
        }

        rendered_lines.push(html! {
            <div class="whitespace-nowrap overflow-hidden">
                {line_elements.into_iter().collect::<Html>()}
            </div>
        });
    }

    let rendered_text = rendered_lines.into_iter().collect::<Html>();

    let progress_pct = if total_chars > 0 {
        (consumed_quote_chars as f64 / total_chars as f64) * 100.0
    } else {
        0.0
    };

    html! {
        <div ref={div_ref} class="w-full px-4 focus:outline-none" tabindex="0" onkeydown={on_keydown} style="max-width: 70vw; margin: 0 auto;">
            <h2 class="text-3xl font-bold mb-4 text-center">{"ThockFlow"}</h2>

            if !*finished {
                <div class="w-full h-1.5 bg-gray-200 rounded-full mb-6 dark:bg-gray-700">
                    <div class="h-1.5 bg-blue-500 rounded-full dark:bg-blue-400 transition-all duration-200 ease-out" style={format!("width: {:.1}%", progress_pct)}></div>
                </div>
                <div class="p-6 bg-gray-100 dark:bg-gray-800 rounded-lg">
                    <div class="text-4xl font-mono select-none" style="line-height: 1.8;">
                        {rendered_text}
                    </div>
                </div>
            } else {
                <div class="mb-8 p-8 bg-gray-100 dark:bg-gray-800 rounded-lg">
                    <h3 class="text-3xl font-bold mb-6 text-center">{"Results"}</h3>

                    // Main stats grid
                    <div class="grid grid-cols-3 gap-4 text-center mb-6">
                        <div>
                            <div class="text-4xl font-bold text-blue-500">{format!("{:.0}", wpm)}</div>
                            <div class="text-gray-600 dark:text-gray-400 text-sm">{"WPM"}</div>
                        </div>
                        <div>
                            <div class="text-4xl font-bold text-cyan-500">{format!("{:.0}", cpm)}</div>
                            <div class="text-gray-600 dark:text-gray-400 text-sm">{"CPM"}</div>
                        </div>
                        <div>
                            <div class="text-4xl font-bold text-green-500">{format!("{:.0}%", accuracy)}</div>
                            <div class="text-gray-600 dark:text-gray-400 text-sm">{"Accuracy"}</div>
                        </div>
                    </div>

                    // Secondary stats
                    <div class="grid grid-cols-4 gap-4 text-center mb-6 text-sm">
                        <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                            <div class="text-xl font-bold">{total_chars}</div>
                            <div class="text-gray-600 dark:text-gray-400">{"Characters"}</div>
                        </div>
                        <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                            <div class="text-xl font-bold">{total_words}</div>
                            <div class="text-gray-600 dark:text-gray-400">{"Words"}</div>
                        </div>
                        <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                            <div class="text-xl font-bold text-red-500">{*error_count}</div>
                            <div class="text-gray-600 dark:text-gray-400">{"Errors"}</div>
                        </div>
                        <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                            <div class="text-xl font-bold">{format!("{:.1}s", elapsed_seconds)}</div>
                            <div class="text-gray-600 dark:text-gray-400">{"Time"}</div>
                        </div>
                    </div>

                    // Timeline chart
                    <div class="mb-4">
                        <div class="text-sm text-gray-500 dark:text-gray-400 mb-2">{"Speed Timeline"}</div>
                        <div class="relative bg-gray-200 dark:bg-gray-700 rounded h-32 overflow-hidden">
                            // Error bar at bottom
                            <div class="absolute bottom-0 left-0 right-0 h-2 flex">
                                {timeline_data.iter().map(|(_, _, _, is_error)| {
                                    let width_pct = 100.0 / timeline_data.len() as f64;
                                    let color = if *is_error { "bg-red-500" } else { "bg-transparent" };
                                    html! {
                                        <div class={color} style={format!("width: {}%;", width_pct)}></div>
                                    }
                                }).collect::<Html>()}
                            </div>

                            // Chart lines
                            <svg class="w-full h-full" viewBox="-15 0 130 100" preserveAspectRatio="none">
                                // Y-axis ticks and labels
                                {y_ticks.iter().map(|(wpm_val, cpm_val)| {
                                    let y_pos = 100.0 - (wpm_val / chart_max * 90.0).min(95.0); // Same scaling as lines
                                    html! {
                                        <>
                                            // Grid line
                                            <line x1="0" y1={format!("{:.1}", y_pos)} x2="100" y2={format!("{:.1}", y_pos)}
                                                  stroke="#4b5563" stroke-width="0.1" stroke-dasharray="0.5,0.5" />
                                            // WPM Label (left)
                                            <text x="-2" y={format!("{:.1}", y_pos + 0.5)} // +0.5 to vertically center text
                                                  font-size="3" fill="#9ca3af" text-anchor="end" alignment-baseline="middle">
                                                {format!("{:.0}", wpm_val)}
                                            </text>
                                            // CPM Label (right)
                                            <text x="102" y={format!("{:.1}", y_pos + 0.5)} // +0.5 to vertically center text
                                                  font-size="3" fill="#9ca3af" text-anchor="start" alignment-baseline="middle">
                                                {format!("{:.0}", cpm_val)}
                                            </text>
                                        </>
                                    }
                                }).collect::<Html>()}

                                // Cumulative WPM line (blue)
                                <polyline
                                    fill="none"
                                    stroke="#3b82f6"
                                    stroke-width="0.5"
                                    points={timeline_data.iter().enumerate().map(|(i, (cum_wpm, _, _, _))| {
                                        let x = (i as f64 / timeline_data.len() as f64) * 100.0;
                                        let y = 100.0 - (cum_wpm / chart_max * 90.0).min(95.0);
                                        format!("{:.1},{:.1}", x, y)
                                    }).collect::<Vec<_>>().join(" ")}
                                />
                                // Instantaneous CPM line (cyan, scaled to WPM equivalent)
                                <polyline
                                    fill="none"
                                    stroke="#06b6d4"
                                    stroke-width="0.3"
                                    stroke-opacity="0.6"
                                    points={timeline_data.iter().enumerate().map(|(i, (_, inst_cpm, _, _))| {
                                        let x = (i as f64 / timeline_data.len() as f64) * 100.0;
                                        let y = 100.0 - ((inst_cpm / 5.0) / chart_max * 90.0).min(95.0);
                                        format!("{:.1},{:.1}", x, y)
                                    }).collect::<Vec<_>>().join(" ")}
                                />
                            </svg>

                            // Legend
                            <div class="absolute top-1 right-1 text-xs flex gap-2">
                                <span class="text-blue-500">{"WPM"}</span>
                                <span class="text-cyan-500">{"CPM"}</span>
                            </div>
                        </div>
                    </div>

                    // Debug Window
                    <div class="mt-8 p-4 bg-gray-200 dark:bg-gray-900 rounded text-xs font-mono overflow-auto max-h-40 whitespace-pre-wrap">
                        <div class="font-bold mb-2 border-b border-gray-400 pb-1">{"Debug Info"}</div>
                        <div class="mb-2"><span class="font-bold text-gray-600 dark:text-gray-400">{"Quote:  "}</span>{(*current_quote).clone()}</div>
                        <div class="mb-2"><span class="font-bold text-gray-600 dark:text-gray-400">{"Input:  "}</span>{(*user_input).clone()}</div>
                        <div class="mb-2"><span class="font-bold text-gray-600 dark:text-gray-400">{"KeyLog: "}</span>{(*key_log).clone()}</div>
                        <div><span class="font-bold text-gray-600 dark:text-gray-400">{"Errors: "}</span>{*error_count}</div>
                    </div>

                    <div class="text-center text-gray-500 dark:text-gray-400 text-sm mt-4">
                        {"Press "}<kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">{"ESC"}</kbd>{" or "}<kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">{"TAB"}</kbd>{" for next quote"}
                    </div>
                </div>
            }
        </div>
    }
}
