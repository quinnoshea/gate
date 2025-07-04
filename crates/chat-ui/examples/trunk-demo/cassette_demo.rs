use gate_chat_ui::{ChatContainer, ChatMessage, ChatResponse, Provider};
use gate_chat_ui::utils::simple_cassette_parser::parse_cassette;
use gate_fixtures::{list_all_cassettes, get_cassette};
use wasm_bindgen::prelude::*;
use web_sys::window;
use yew::prelude::*;

#[derive(Clone, PartialEq)]
struct CassetteInfo {
    provider: String,
    name: String,
    display_name: String,
}

#[function_component(App)]
fn app() -> Html {
    let dark_mode = use_state(|| {
        window()
            .and_then(|w| w.document())
            .and_then(|d| d.document_element())
            .map(|e| e.class_list().contains("dark"))
            .unwrap_or(false)
    });

    let toggle_dark_mode = {
        let dark_mode = dark_mode.clone();
        Callback::from(move |_| {
            let new_state = !*dark_mode;
            dark_mode.set(new_state);

            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(element) = document.document_element() {
                        let class_list = element.class_list();
                        if new_state {
                            let _ = class_list.add_1("dark");
                        } else {
                            let _ = class_list.remove_1("dark");
                        }
                    }
                }
            }
        })
    };

    // Get available cassettes
    let available_cassettes = use_memo((), |_| {
        let mut cassettes = Vec::new();
        let all = list_all_cassettes();
        
        for (provider, names) in all {
            for name in names {
                let display_name = name
                    .replace("_", " ")
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                
                cassettes.push(CassetteInfo {
                    provider: provider.to_string(),
                    name: name.to_string(),
                    display_name,
                });
            }
        }
        
        cassettes.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.name.cmp(&b.name)));
        cassettes
    });

    let selected_cassette = use_state(|| None::<CassetteInfo>);
    let loading_error = use_state(|| None::<String>);
    
    let chat_response = use_state(|| ChatResponse {
        id: "demo-1".to_string(),
        model: "demo-model".to_string(),
        messages: vec![
            ChatMessage::system("Welcome to the Gate Chat UI Cassette Demo!\\n\\nThis demo allows you to load real API conversation cassettes and view them in the chat interface. Select a cassette from the dropdown above to see a real conversation."),
        ],
        provider: Provider::Unknown("demo".to_string()),
        usage: None,
        metadata: Default::default(),
    });

    let on_cassette_select = {
        let selected_cassette = selected_cassette.clone();
        let chat_response = chat_response.clone();
        let loading_error = loading_error.clone();
        
        Callback::from(move |e: Event| {
            loading_error.set(None);
            
            let target = e.target().unwrap();
            let select = target.dyn_into::<web_sys::HtmlSelectElement>().unwrap();
            let value = select.value();
            
            if value.is_empty() {
                selected_cassette.set(None);
                // Reset to welcome message
                chat_response.set(ChatResponse {
                    id: "demo-1".to_string(),
                    model: "demo-model".to_string(),
                    messages: vec![
                        ChatMessage::system("Welcome to the Gate Chat UI Cassette Demo!\\n\\nThis demo allows you to load real API conversation cassettes and view them in the chat interface. Select a cassette from the dropdown above to see a real conversation."),
                    ],
                    provider: Provider::Unknown("demo".to_string()),
                    usage: None,
                    metadata: Default::default(),
                });
                return;
            }
            
            // Parse provider:name format
            if let Some((provider, name)) = value.split_once(':') {
                let cassette_info = CassetteInfo {
                    provider: provider.to_string(),
                    name: name.to_string(),
                    display_name: name.replace("_", " "),
                };
                
                selected_cassette.set(Some(cassette_info.clone()));
                
                // Load the cassette
                if let Some(cassette_content) = get_cassette(provider, name) {
                    match parse_cassette(cassette_content) {
                        Ok(mut parsed_response) => {
                            // Add a header message explaining what cassette is loaded
                            let header = ChatMessage::system(format!(
                                "Loaded cassette: {} - {}\\n\\nThis is a real API conversation captured for testing.",
                                provider.to_uppercase(),
                                cassette_info.display_name
                            ));
                            parsed_response.messages.insert(0, header);
                            
                            chat_response.set(parsed_response);
                        }
                        Err(e) => {
                            loading_error.set(Some(format!("Failed to parse cassette: {}", e)));
                        }
                    }
                } else {
                    loading_error.set(Some("Failed to load cassette content".to_string()));
                }
            }
        })
    };

    html! {
        <div class="h-screen flex flex-col">
            <div class="bg-blue-600 dark:bg-gray-800 text-white p-4 shadow-lg">
                <div class="flex justify-between items-center mb-3">
                    <h1 class="text-2xl font-bold">{"Gate Chat UI - Cassette Demo"}</h1>
                    <button
                        onclick={toggle_dark_mode}
                        class="p-2 rounded-lg bg-white/10 hover:bg-white/20 transition-colors"
                        title={if *dark_mode { "Switch to light mode" } else { "Switch to dark mode" }}
                    >
                        if *dark_mode {
                            // Sun icon for light mode
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12 18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z" />
                            </svg>
                        } else {
                            // Moon icon for dark mode
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 009.002-5.998z" />
                            </svg>
                        }
                    </button>
                </div>
                <div class="flex items-center gap-3">
                    <label for="cassette-select" class="text-sm">{"Load Cassette:"}</label>
                    <select
                        id="cassette-select"
                        onchange={on_cassette_select}
                        class="flex-1 px-3 py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors cursor-pointer"
                    >
                        <option value="" class="bg-gray-800">{"Select a cassette..."}</option>
                        {
                            available_cassettes.iter().map(|cassette| {
                                let value = format!("{}:{}", cassette.provider, cassette.name);
                                let label = format!("{} - {}", 
                                    cassette.provider.to_uppercase(), 
                                    cassette.display_name
                                );
                                html! {
                                    <option value={value} class="bg-gray-800">{label}</option>
                                }
                            }).collect::<Html>()
                        }
                    </select>
                </div>
                if let Some(error) = &*loading_error {
                    <div class="mt-2 text-sm text-red-300">
                        {error}
                    </div>
                }
            </div>
            <div class="flex-1 overflow-hidden">
                <ChatContainer
                    chat_response={(*chat_response).clone()}
                    show_metadata={true}
                    show_input={false}
                    allow_images={false}
                    allow_files={false}
                    input_disabled={true}
                />
            </div>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    yew::Renderer::<App>::new().render();
}

fn main() {
    // Empty main for wasm32 target
}