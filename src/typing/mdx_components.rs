use std::borrow::Borrow;
use yew::{function_component, html, Children, Html, Properties};

#[derive(PartialEq, Properties)]
pub struct ChildProps {
    #[prop_or_default]
    pub children: Children,
}

#[allow(dead_code)]
const HEADER_LINK_LEN: usize = 20;

#[allow(dead_code)]
#[function_component]
pub fn MyH1(c: &ChildProps) -> Html {
    let tag = children_to_slug(c.children.iter());
    html! {
      <h1 id={tag.clone()} class="text-4xl pt-10 pb-6">
        <a class="text-inherit" href={format!("#{tag}")}>
          {c.children.clone()}
        </a>
      </h1>
    }
}

#[allow(dead_code)]
#[function_component]
pub fn MyH2(c: &ChildProps) -> Html {
    let tag = children_to_slug(c.children.iter());
    html! {
      <h2 id={tag.clone()} class="text-2xl pt-8 pb-4">
        <a class="text-inherit" href={format!("#{tag}")}>
          {c.children.clone()}
        </a>
      </h2>
    }
}

#[allow(dead_code)]
#[function_component]
pub fn MyH3(c: &ChildProps) -> Html {
    let tag = children_to_slug(c.children.iter());
    html! {
      <h3 id={tag.clone()} class="text-xl pt-6 pb-2">
        <a class="text-inherit" href={format!("#{tag}")}>
          {c.children.clone()}
        </a>
      </h3>
    }
}

#[allow(dead_code)]
fn children_to_slug<H: Borrow<Html>>(c: impl IntoIterator<Item = H>) -> String {
    let mut out = children_to_string(c);
    out.truncate(HEADER_LINK_LEN);
    out
}

#[allow(dead_code)]
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
pub fn MyPre(c: &ChildProps) -> Html {
    html! {
      <pre class="overflow-auto m-4 p-6 bg-gray-300/5 rounded">
      {c.children.clone()}
      </pre>
    }
}

#[function_component]
pub fn MyBlockquote(c: &ChildProps) -> Html {
    html! {
      <blockquote class="text-black/70 dark:text-white/50 border-l-8 px-2 my-2 italic">
        {c.children.clone()}
      </blockquote>
    }
}

#[function_component]
pub fn MyP(c: &ChildProps) -> Html {
    html! {
      <p class="py-2 text-lg">
        {c.children.clone()}
      </p>
    }
}

#[function_component]
pub fn MyUl(c: &ChildProps) -> Html {
    html! {
      <div class="px-4">{c.children.clone()}</div>
    }
}

#[function_component]
pub fn MyLi(c: &ChildProps) -> Html {
    html! {
      <p class="py-1">{" - "}{c.children.clone()}</p>
    }
}

#[function_component]
pub fn MyCode(c: &ChildProps) -> Html {
    html! {
      <code class="bg-gray-300/40 dark:bg-gray-300/20 px-1 rounded">
        {c.children.clone()}
      </code>
    }
}

#[macro_export]
macro_rules! typing_style {
    () => {
        use crate::typing::mdx_components::*;
        yew::mdx_style!(
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

#[derive(Properties, PartialEq)]
pub struct HeyProps {
    pub name: String,
}
#[function_component]
pub fn HeyComponent(props: &HeyProps) -> Html {
    let count = yew::use_state(|| 0);
    let s = count.to_string();

    html! {
        <>
        <p>{"said hey to "}{&props.name}{" "} <em onclick={ move |_| count.set(*count + 1)}>{s}</em>
        {" times"}</p>
        </>
    }
}

#[function_component]
pub fn Counter() -> Html {
    let count = yew::use_state(|| 0);
    let click = yew::use_callback(|_, [count]| count.set(**count + 1), [count.clone()]);

    html! {
        <button onclick={ click } class="bg-gray-300/30 rounded select-none p-2">
        {"Counter "}{*count}
        </button>
    }
}
