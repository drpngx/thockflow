mod typing;

use std::collections::HashMap;

use yew::prelude::*;
use yew_router::{
    history::{AnyHistory, History, MemoryHistory},
    prelude::*,
};

#[derive(Clone, PartialEq, Debug)]
pub struct QuoteContext {
    pub index: usize,
}

#[derive(Routable, PartialEq, Clone)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/typing")]
    Typing,
}

#[derive(Properties, PartialEq, Debug, Default)]
pub struct AppProps {
    pub init_quote_index: Option<usize>,
}

#[function_component]
pub fn App(props: &AppProps) -> Html {
    let context = props.init_quote_index.map(|i| QuoteContext { index: i });
    
    html! {
        <ContextProvider<Option<QuoteContext>> context={context}>
            <BrowserRouter>
                <Switch<Route> render={switch} />
            </BrowserRouter>
        </ContextProvider<Option<QuoteContext>>>
    }
}

#[derive(Properties, PartialEq, Debug)]
pub struct ServerAppProps {
    pub path: String,
    pub queries: HashMap<String, String>,
    pub init_quote_index: Option<usize>,
}

#[function_component]
pub fn ServerApp(props: &ServerAppProps) -> Html {
    let history = AnyHistory::from(MemoryHistory::new());
    history
        .push_with_query(&*props.path, &props.queries)
        .unwrap();
        
    let context = props.init_quote_index.map(|i| QuoteContext { index: i });

    html! {
        <ContextProvider<Option<QuoteContext>> context={context}>
            <Router history={history}>
                <Switch<Route> render={switch} />
            </Router>
        </ContextProvider<Option<QuoteContext>>>
    }
}

#[function_component]
fn Navbar() -> Html {
    html! {
        <div class="flex justify-evenly flex-wrap w-full navbar">
            <h1 class="font-display text-6xl p-10">
                <Link<Route> to={Route::Home}><div class="p-4">{"ThockFlow"}</div></Link<Route>>
            </h1>
            <div class="flex items-center flex-wrap">
                <Link<Route> classes="p-4 text-3xl  " to={Route::Typing}>
                    <button >
                        {"Typing"}
                    </button>
                </Link<Route>>
                <a class="p-4 text-3xl" href="https://github.com/drpngx/thockflow">{"GitHub"}</a>
            </div>
        </div>
    }
}

#[function_component]
fn Home() -> Html {
    html! {}
}

fn switch(route: Route) -> Html {
    html! {
        <main class="font-body flex flex-col items-start px-2 bg-white dark:bg-gray-900 text-black dark:text-white min-h-screen">
            <Navbar />
            {
                match route {
                    Route::Home => html! {
                        <>
                        <div class="w-full font-body text-3xl my-10 flex justify-evenly px-2 flex-wrap">
                            <div class="max-w-md">
                                <div>
                                    <strong class="font-extrabold">{"ThockFLow"}</strong>
                                </div>
                                <div class="">
                                    {"Satisfying typing practice"}
                                </div>
                            </div>
                            <div></div>
                            <div></div>
                        </div>
                        <div class="w-full font-body text-3xl my-10 flex justify-evenly px-2 flex-wrap">
                            <div class="w-full text-xl max-w-sm">
                                {"This is character-based typing practice with more quotes."}
                            </div>
                            <div></div>
                            <div></div>
                        </div>
                        </>
                    },
                    Route::Typing => html! {
                        <div class="w-full font-body flex px-2 flex-col items-center place-content-around">
                            <div class="flex flex-col">
                                <typing::TypingHome />
                            </div>
                        </div>
                    },
                }
            }
        </main>
    }
}
