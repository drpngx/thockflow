fn main() {
    #[cfg(target_arch = "wasm32")]
    wasm_logger::init(wasm_logger::Config::new(log::Level::Trace));
    let init_quote_index = web_sys::window()
        .and_then(|w| w.get("THOCKFLOW_INIT_INDEX"))
        .and_then(|v| v.as_f64())
        .map(|v| v as usize);
        
    yew::Renderer::<thockflow::App>::with_props(thockflow::AppProps { init_quote_index }).hydrate();
}
