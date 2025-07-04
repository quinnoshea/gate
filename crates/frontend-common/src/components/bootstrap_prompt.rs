//! Bootstrap prompt component for initial admin setup

use crate::services::BootstrapStatus;
use yew::prelude::*;

/// Props for the bootstrap prompt component
#[derive(Properties, PartialEq)]
pub struct BootstrapPromptProps {
    /// Callback when bootstrap token is entered
    pub on_token: Callback<String>,
    /// Whether to show the prompt
    pub show: bool,
    /// Bootstrap status (optional, passed from container)
    #[prop_or_default]
    pub bootstrap_status: Option<BootstrapStatus>,
    /// Bootstrap token (optional, only shown when provided)
    #[prop_or_default]
    pub bootstrap_token: Option<String>,
    /// Whether token is being loaded
    #[prop_or_default]
    pub is_loading_token: bool,
    /// Token loading error
    #[prop_or_default]
    pub token_error: Option<String>,
}

/// Bootstrap prompt component
#[function_component(BootstrapPrompt)]
pub fn bootstrap_prompt(props: &BootstrapPromptProps) -> Html {
    let token = use_state(String::new);
    let error = use_state(|| Option::<String>::None);

    // Handle token input
    let on_token_input = {
        let token = token.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            token.set(input.value());
        })
    };

    // Submit token
    let on_submit = {
        let token = token.clone();
        let on_token = props.on_token.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if !token.is_empty() {
                on_token.emit((*token).clone());
            }
        })
    };

    if !props.show {
        return html! {};
    }

    html! {
        <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
            <div class="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md w-full mx-4 shadow-xl">
                <h2 class="text-2xl font-bold mb-4 text-gray-800 dark:text-white">
                    {"Initial Admin Setup"}
                </h2>

                {if let Some(status) = &props.bootstrap_status {
                    html! {
                        <div class="mb-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                            <p class="text-sm text-blue-800 dark:text-blue-200">
                                {&status.message}
                            </p>
                        </div>
                    }
                } else {
                    html! {}
                }}

                <p class="text-gray-600 dark:text-gray-300 mb-6">
                    {"This is the first time Gate is being set up. Please enter the bootstrap token to register as the admin user."}
                </p>

                // Show bootstrap token if available
                {if let Some(token) = &props.bootstrap_token {
                    html! {
                        <div class="mb-6 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                            <p class="text-xs font-semibold text-blue-800 dark:text-blue-200 mb-2">
                                {"Bootstrap Token (from Tauri app):"}
                            </p>
                            <p class="font-mono text-sm break-all select-all text-blue-900 dark:text-blue-100 bg-white dark:bg-gray-800 p-3 rounded">
                                {token}
                            </p>
                            <p class="text-xs text-blue-700 dark:text-blue-300 mt-2">
                                {"⚠️ This token is single-use only. Copy it now or enter it below."}
                            </p>
                        </div>
                    }
                } else if props.is_loading_token {
                    html! {
                        <div class="mb-4 p-3 bg-gray-100 dark:bg-gray-800 rounded-lg text-center">
                            <p class="text-sm text-gray-600 dark:text-gray-400">
                                {"Loading bootstrap token..."}
                            </p>
                        </div>
                    }
                } else {
                    html! {}
                }}

                <form onsubmit={on_submit}>
                    <div class="mb-4">
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            {"Bootstrap Token"}
                        </label>
                        <input
                            type="text"
                            class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                            placeholder="Enter bootstrap token"
                            value={(*token).clone()}
                            oninput={on_token_input}
                            required=true
                        />
                    </div>


                    {if let Some(error_msg) = props.token_error.as_ref().or((*error).as_ref()) {
                        html! {
                            <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 rounded-lg">
                                <p class="text-sm text-red-800 dark:text-red-200">
                                    {error_msg}
                                </p>
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    <button
                        type="submit"
                        disabled={token.is_empty()}
                        class="w-full px-4 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed font-medium"
                    >
                        {"Continue with Setup"}
                    </button>
                </form>

                <div class="mt-6 p-4 bg-amber-50 dark:bg-amber-900/20 rounded-lg">
                    <p class="text-xs text-amber-800 dark:text-amber-200">
                        <strong>{"Note:"}</strong> {" The bootstrap token is single-use and will be invalidated after the first admin user is created."}
                    </p>
                </div>
            </div>
        </div>
    }
}
