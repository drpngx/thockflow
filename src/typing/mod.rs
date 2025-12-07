use std::borrow::Borrow;

use wasm_bindgen::JsCast;
use yew::{
    function_component, html, include_mdx, mdx, mdx_style, use_callback, use_effect, use_node_ref, use_state, Callback, Children, Html,
    Properties,
};
use yew_router::prelude::Link;

use crate::Route;

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

// Calculate Levenshtein distance (edit distance) between two strings
fn edit_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
                matrix[i][j] + cost,
            );
        }
    }

    matrix[len1][len2]
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
        let div_ref = div_ref.clone();

        Callback::from(move |_| {
            let idx = (js_sys::Math::random() * quotes::QUOTES.len() as f64) as usize;
            current_quote.set(quotes::QUOTES[idx].to_string());
            user_input.set(String::new());
            current_position.set(0);
            started.set(false);
            finished.set(false);
            start_time.set(None);
            end_time.set(None);
            error_count.set(0);

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
                }
                return;
            }

            // Only process single character keys
            if key.len() != 1 {
                return;
            }

            e.prevent_default();

            if !*started {
                started.set(true);
                start_time.set(Some(js_sys::Date::now()));
            }

            let mut current = (*user_input).clone();
            current.push_str(&key);

            // Check if this character is incorrect
            let quote_chars: Vec<char> = current_quote.chars().collect();
            if let Some(&expected_char) = quote_chars.get(*current_position) {
                if key.chars().next() != Some(expected_char) {
                    error_count.set(*error_count + 1);
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
    let (wpm, accuracy) = if *finished {
        if let (Some(start), Some(end)) = (*start_time, *end_time) {
            let elapsed_minutes = (end - start) / 1000.0 / 60.0;
            let char_count = current_quote.chars().count();
            let wpm = (char_count as f64 / 5.0) / elapsed_minutes; // Standard: 5 chars = 1 word

            let distance = edit_distance(&current_quote, &user_input);
            let accuracy = ((char_count.saturating_sub(distance)) as f64 / char_count as f64 * 100.0).max(0.0);

            (wpm, accuracy)
        } else {
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    };

    // Render each character with color coding
    let rendered_text = current_quote.chars().enumerate().map(|(i, quote_char)| {
        let user_chars: Vec<char> = user_input.chars().collect();
        let (class, show_cursor, typed_char_wrong) = if i < user_chars.len() {
            // Already typed
            if user_chars[i] == quote_char {
                // Correct
                ("text-white dark:text-white", false, None)
            } else {
                // Incorrect - show what was typed
                ("text-red-500 dark:text-red-400 bg-red-900/30", false, Some(user_chars[i]))
            }
        } else if i == *current_position && !*finished {
            // Current position - show cursor
            ("text-gray-500 dark:text-gray-500", true, None)
        } else {
            // Not yet typed
            ("text-gray-500 dark:text-gray-500", false, None)
        };

        html! {
            <span class="relative inline-block" style="min-width: 0.6em;">
                <span class={class}>
                    {if show_cursor {
                        html! {
                            <span class="absolute left-0 top-0 h-full w-0.5 bg-yellow-400 animate-pulse" style="margin-left: -2px;"></span>
                        }
                    } else {
                        html! {}
                    }}
                    {quote_char}
                </span>
                {if let Some(ch) = typed_char_wrong {
                    html! {
                        <span class="absolute text-sm font-bold text-red-300 dark:text-red-200" style="left: 50%; transform: translateX(-50%); top: 1.8em; white-space: nowrap;">
                            {ch}
                        </span>
                    }
                } else {
                    html! {}
                }}
            </span>
        }
    }).collect::<Html>();

    html! {
        <div ref={div_ref} class="w-full max-w-4xl mx-auto p-8 focus:outline-none" tabindex="0" onkeydown={on_keydown}>
            <h2 class="text-4xl font-bold mb-8 text-center">{"ThockFlow"}</h2>

            if !*finished {
                <div class="mb-8 p-8 bg-gray-100 dark:bg-gray-800 rounded-lg">
                    <div class="text-3xl font-mono select-none" style="line-height: 3em;">
                        {rendered_text}
                    </div>
                </div>
            } else {
                <div class="mb-8 p-8 bg-gray-100 dark:bg-gray-800 rounded-lg">
                    <h3 class="text-3xl font-bold mb-6 text-center">{"Results"}</h3>
                    <div class="grid grid-cols-3 gap-8 text-center mb-6">
                        <div>
                            <div class="text-5xl font-bold text-blue-500">{format!("{:.0}", wpm)}</div>
                            <div class="text-gray-600 dark:text-gray-400 mt-2">{"WPM"}</div>
                        </div>
                        <div>
                            <div class="text-5xl font-bold text-green-500">{format!("{:.0}%", accuracy)}</div>
                            <div class="text-gray-600 dark:text-gray-400 mt-2">{"Accuracy"}</div>
                        </div>
                        <div>
                            <div class="text-5xl font-bold text-red-500">{*error_count}</div>
                            <div class="text-gray-600 dark:text-gray-400 mt-2">{"Errors"}</div>
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
