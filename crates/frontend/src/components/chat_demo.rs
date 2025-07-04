use gate_chat_ui::{ChatContainer, parse_cassette, types::ChatResponse};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct CassetteInfo {
    pub provider: &'static str,
    pub name: String,
    pub content: &'static str,
}

// Include the generated cassette list
include!(concat!(env!("OUT_DIR"), "/cassette_list.rs"));

#[function_component(ChatDemo)]
pub fn chat_demo() -> Html {
    let selected_cassette = use_state(|| None::<usize>);
    let chat_response = use_state(|| None::<ChatResponse>);
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let cassettes = get_all_cassettes();

    // Debug: Log cassette information
    web_sys::console::log_1(&format!("Total cassettes loaded: {}", cassettes.len()).into());
    if !cassettes.is_empty() {
        web_sys::console::log_1(
            &format!(
                "First cassette: [{}] {}",
                cassettes[0].provider, cassettes[0].name
            )
            .into(),
        );
        if cassettes.len() > 50 {
            web_sys::console::log_1(
                &format!(
                    "Cassette at index 50: [{}] {}",
                    cassettes[50].provider, cassettes[50].name
                )
                .into(),
            );
        }
        // Find first Anthropic cassette
        if let Some((idx, cassette)) = cassettes
            .iter()
            .enumerate()
            .find(|(_, c)| c.provider == "Anthropic")
        {
            web_sys::console::log_1(
                &format!(
                    "First Anthropic cassette at index {}: [{}] {}",
                    idx, cassette.provider, cassette.name
                )
                .into(),
            );
        }
    }

    let load_cassette = {
        let selected_cassette = selected_cassette.clone();
        let chat_response = chat_response.clone();
        let loading = loading.clone();
        let error = error.clone();
        let cassettes = cassettes.clone();

        Callback::from(move |idx: usize| {
            web_sys::console::log_1(
                &format!("Load cassette callback called with index: {idx}").into(),
            );

            if idx >= cassettes.len() {
                web_sys::console::log_1(
                    &format!(
                        "ERROR: Index {} out of bounds (total: {})",
                        idx,
                        cassettes.len()
                    )
                    .into(),
                );
                return;
            }

            // Update the selected cassette state
            selected_cassette.set(Some(idx));

            let cassette = &cassettes[idx];
            let cassette_content = cassette.content.to_string();
            let cassette_name = cassette.name.clone();
            let chat_response = chat_response.clone();
            let loading = loading.clone();
            let error = error.clone();

            spawn_local(async move {
                loading.set(true);
                error.set(None);

                web_sys::console::log_1(&format!("Loading cassette: {cassette_name}").into());
                web_sys::console::log_1(
                    &format!("Cassette content length: {} chars", cassette_content.len()).into(),
                );

                // Parse the JSON cassette content
                match parse_cassette(&cassette_content) {
                    Ok(parsed_response) => {
                        web_sys::console::log_1(&format!("Successfully parsed cassette. Provider: {:?}, Messages: {}, Model: {}", 
                            parsed_response.provider, parsed_response.messages.len(), parsed_response.model).into());
                        if !parsed_response.messages.is_empty() {
                            web_sys::console::log_1(
                                &format!(
                                    "First message: {:?} - '{}'",
                                    parsed_response.messages[0].role,
                                    parsed_response.messages[0]
                                        .content
                                        .as_ref()
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("No text")
                                )
                                .into(),
                            );
                        }
                        chat_response.set(Some(parsed_response));
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("ERROR parsing cassette {cassette_name}: {e}").into(),
                        );
                        error.set(Some(format!("Failed to parse {cassette_name}: {e}")));
                    }
                }

                loading.set(false);
            });
        })
    };

    let on_select_change = {
        let load_cassette = load_cassette.clone();
        let cassettes = cassettes.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let value = select.value();
                web_sys::console::log_1(
                    &format!(
                        "Dropdown selection: value='{}', empty={}",
                        value,
                        value.is_empty()
                    )
                    .into(),
                );
                if !value.is_empty() {
                    if let Ok(idx) = value.parse::<usize>() {
                        web_sys::console::log_1(&format!("Parsed index: {idx}").into());
                        if idx < cassettes.len() {
                            web_sys::console::log_1(
                                &format!(
                                    "Will load cassette at index {}: [{}] {}",
                                    idx, cassettes[idx].provider, cassettes[idx].name
                                )
                                .into(),
                            );
                            // Pass the index directly to the callback
                            load_cassette.emit(idx);
                        } else {
                            web_sys::console::log_1(
                                &format!(
                                    "ERROR: Index {} out of bounds (total: {})",
                                    idx,
                                    cassettes.len()
                                )
                                .into(),
                            );
                        }
                    } else {
                        web_sys::console::log_1(
                            &format!("ERROR: Failed to parse index from value '{value}'").into(),
                        );
                    }
                }
            }
        })
    };

    // Don't load initial cassette - let user select one

    html! {
        <div class="flex h-[calc(100vh-100px)] gap-4 p-4 bg-gray-100 dark:bg-gray-900">
            <div class="w-[300px] bg-white dark:bg-gray-800 rounded-lg p-5 shadow-md overflow-y-auto">
                <h2 class="text-xl font-bold text-gray-800 dark:text-gray-200 mb-4">{"Chat UI Demo"}</h2>
                <p class="text-gray-700 dark:text-gray-300 mb-2">{"Select a cassette to view:"}</p>

                <select class="w-full p-2 mb-4 border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-200 rounded text-sm" onchange={on_select_change}>
                    <option value="" disabled=true selected={selected_cassette.is_none()}>
                        {"-- Select a cassette --"}
                    </option>
                    {for cassettes.iter().enumerate().map(|(idx, cassette)| {
                        html! {
                            <option value={idx.to_string()} selected={*selected_cassette == Some(idx)}>
                                {format!("[{}] {}", cassette.provider, cassette.name)}
                            </option>
                        }
                    })}
                </select>

                if *loading {
                    <div class="text-center text-gray-600 dark:text-gray-400 py-2">{"Loading..."}</div>
                }

                if let Some(err) = &*error {
                    <div class="bg-red-50 dark:bg-red-900 text-red-700 dark:text-red-300 p-2 rounded my-2">{err}</div>
                }

                <div class="mt-6 pt-6 border-t border-gray-200 dark:border-gray-700">
                    <h3 class="text-base font-semibold text-gray-700 dark:text-gray-300 mb-3">{"About this demo"}</h3>
                    <p class="text-sm text-gray-600 dark:text-gray-400 mb-2 leading-relaxed">{"This demo renders real LLM API responses from the sample cassettes, parsed and displayed with the chat UI component."}</p>
                    <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">{"Features demonstrated:"}</p>
                    <ul class="list-disc pl-5">
                        <li class="text-sm text-gray-600 dark:text-gray-400 my-1">{"OpenAI, Anthropic & Google formats"}</li>
                        <li class="text-sm text-gray-600 dark:text-gray-400 my-1">{"Multi-turn conversations"}</li>
                        <li class="text-sm text-gray-600 dark:text-gray-400 my-1">{"Streaming responses"}</li>
                        <li class="text-sm text-gray-600 dark:text-gray-400 my-1">{"Tool/function calls"}</li>
                        <li class="text-sm text-gray-600 dark:text-gray-400 my-1">{"Markdown formatting"}</li>
                        <li class="text-sm text-gray-600 dark:text-gray-400 my-1">{"Usage metadata display"}</li>
                    </ul>
                </div>
            </div>

            <div class="flex-1 bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden flex flex-col">
                {if let Some(response) = &*chat_response {
                    html! {
                        <ChatContainer
                            chat_response={response.clone()}
                            show_metadata={true}
                            show_input={true}
                            on_send_message={Callback::from(move |msg: String| {
                                web_sys::console::log_1(&format!("User sent: {msg}").into());
                            })}
                        />
                    }
                } else {
                    html! {
                        <div class="flex-1 flex items-center justify-center text-gray-400 dark:text-gray-500 text-base">
                            {"Select a cassette to view the conversation"}
                        </div>
                    }
                }}
            </div>

        </div>
    }
}
