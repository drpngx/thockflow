use std::borrow::Borrow;

use yew::{
    function_component, html, mdx_style, use_callback, use_effect, use_node_ref, use_state, Callback, Children, Html,
    Properties,
};

mod quotes;

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

// Edit operation for alignment
#[derive(Clone, Copy, PartialEq)]
enum EditOp {
    Match,      // Characters match
    Substitute, // Wrong character typed
    Insert,     // Extra character in user input
}

// Greedy incremental alignment - only looks 1-2 chars ahead for local errors
// This is O(n) and progresses through the quote gradually
fn align_incremental(quote: &str, input: &str) -> Vec<(EditOp, Option<char>, Option<char>)> {
    let quote_chars: Vec<char> = quote.chars().collect();
    let input_chars: Vec<char> = input.chars().collect();

    let mut result = Vec::new();
    let mut q_idx = 0;
    let mut i_idx = 0;

    while i_idx < input_chars.len() && q_idx < quote_chars.len() {
        if input_chars[i_idx] == quote_chars[q_idx] {
            // Direct match
            result.push((EditOp::Match, Some(quote_chars[q_idx]), Some(input_chars[i_idx])));
            q_idx += 1;
            i_idx += 1;
        } else {
            // Check if next input char matches current quote char (insertion - extra char typed)
            let is_insertion = i_idx + 1 < input_chars.len()
                && input_chars[i_idx + 1] == quote_chars[q_idx];

            // Check if current input matches next quote char (user skipped a quote char)
            let is_skip = q_idx + 1 < quote_chars.len()
                && input_chars[i_idx] == quote_chars[q_idx + 1];

            if is_insertion && !is_skip {
                // Extra character typed
                result.push((EditOp::Insert, None, Some(input_chars[i_idx])));
                i_idx += 1;
            } else if is_skip && !is_insertion {
                // User skipped a quote char - mark the skipped char as missed (no input for it)
                result.push((EditOp::Substitute, Some(quote_chars[q_idx]), None));
                q_idx += 1;
                // Don't advance i_idx - it will match the next quote char in the next iteration
            } else {
                // Simple substitution (typo)
                result.push((EditOp::Substitute, Some(quote_chars[q_idx]), Some(input_chars[i_idx])));
                q_idx += 1;
                i_idx += 1;
            }
        }
    }

    // Handle remaining input chars as insertions (typed beyond quote)
    while i_idx < input_chars.len() {
        result.push((EditOp::Insert, None, Some(input_chars[i_idx])));
        i_idx += 1;
    }

    result
}

// Simple edit distance for final accuracy calculation
fn edit_distance(s1: &str, s2: &str) -> usize {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for i in 0..=a.len() { dp[i][0] = i; }
    for j in 0..=b.len() { dp[0][j] = j; }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i-1] == b[j-1] { 0 } else { 1 };
            dp[i][j] = (dp[i-1][j] + 1).min(dp[i][j-1] + 1).min(dp[i-1][j-1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

#[function_component]
pub fn TypingHome() -> Html {
    let current_quote = use_state(|| {
        let idx = (js_sys::Math::random() * quotes::QUOTES.len() as f64) as usize;
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
        let reset = reset.clone();

        Callback::from(move |e: web_sys::KeyboardEvent| {
            let key = e.key();

            // ESC or Tab always starts new quote
            if key == "Escape" || key == "Tab" {
                e.prevent_default();
                reset.emit(());
                return;
            }

            // If finished, don't process other keys
            if *finished {
                return;
            }

            // Handle backspace
            if key == "Backspace" {
                e.prevent_default();
                if *current_position > 0 {
                    let mut current = (*user_input).clone();
                    current.pop();
                    user_input.set(current);
                    current_position.set(*current_position - 1);
                    // Remove the last keystroke time
                    let mut times = (*keystroke_times).clone();
                    times.pop();
                    keystroke_times.set(times);
                }
                return;
            }

            // Only process single character keys
            if key.len() != 1 {
                return;
            }

            e.prevent_default();

            let now = js_sys::Date::now();

            if !*started {
                started.set(true);
                start_time.set(Some(now));
            }

            // Record keystroke time
            let mut times = (*keystroke_times).clone();
            times.push(now);
            keystroke_times.set(times);

            let mut current = (*user_input).clone();
            current.push_str(&key);

            // Check if this character is incorrect
            let quote_chars: Vec<char> = current_quote.chars().collect();
            if let Some(&expected_char) = quote_chars.get(*current_position) {
                if key.chars().next() != Some(expected_char) {
                    error_count.set(*error_count + 1);
                    // Record error position
                    let mut errors = (*error_positions).clone();
                    errors.push(*current_position);
                    error_positions.set(errors);
                }
            }

            user_input.set(current.clone());

            // Calculate new position
            let new_position = *current_position + 1;
            current_position.set(new_position);

            // Check if finished - when we've reached the end of the quote
            if new_position >= quote_chars.len() {
                finished.set(true);
                end_time.set(Some(js_sys::Date::now()));
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
            let cpm = total_chars as f64 / elapsed_min;
            let wpm = (total_chars as f64 / 5.0) / elapsed_min; // Standard: 5 chars = 1 word

            let distance = edit_distance(&current_quote, &user_input);
            let accuracy = ((total_chars.saturating_sub(distance)) as f64 / total_chars as f64 * 100.0).max(0.0);

            (wpm, cpm, accuracy, elapsed_sec)
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

    html! {
        <div ref={div_ref} class="w-full px-4 focus:outline-none" tabindex="0" onkeydown={on_keydown} style="max-width: 70vw; margin: 0 auto;">
            <h2 class="text-3xl font-bold mb-4 text-center">{"ThockFlow"}</h2>

            if !*finished {
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
                            <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
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

                    <div class="text-center text-gray-500 dark:text-gray-400 text-sm">
                        {"Press "}<kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">{"ESC"}</kbd>{" or "}<kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">{"TAB"}</kbd>{" for next quote"}
                    </div>
                </div>
            }
        </div>
    }
}
