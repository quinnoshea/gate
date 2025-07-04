use crate::utils::is_tauri;
use gate_frontend_common::{
    components::Spinner as LoadingSpinner,
    hooks::{use_webauthn, WebAuthnState},
};
use wasm_bindgen::prelude::*;
use yew::prelude::*;

/// Fetch bootstrap token from Tauri API
async fn fetch_bootstrap_token() -> Result<Option<String>, String> {
    // Only available in Tauri context
    if !is_tauri() {
        return Ok(None);
    }

    // Call the Tauri command directly
    #[wasm_bindgen(inline_js = "
    export async function call_get_bootstrap_token() {
        try {
            if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                return await window.__TAURI_INTERNALS__.invoke('get_bootstrap_token');
            } else if (window.__TAURI__ && window.__TAURI__.invoke) {
                return await window.__TAURI__.invoke('get_bootstrap_token');
            } else if (window.__TAURI__ && window.__TAURI__.tauri && window.__TAURI__.tauri.invoke) {
                return await window.__TAURI__.tauri.invoke('get_bootstrap_token');
            }
            return null;
        } catch (e) {
            console.error('Failed to get bootstrap token:', e);
            throw e;
        }
    }
    ")]
    extern "C" {
        #[wasm_bindgen(catch)]
        async fn call_get_bootstrap_token() -> Result<JsValue, JsValue>;
    }

    match call_get_bootstrap_token().await {
        Ok(result) => {
            if result.is_null() || result.is_undefined() {
                Ok(None)
            } else if result.is_string() {
                Ok(Some(result.as_string().unwrap()))
            } else {
                // Try to parse as JSON
                match serde_wasm_bindgen::from_value::<Option<String>>(result) {
                    Ok(token) => Ok(token),
                    Err(_) => Err("Invalid response format".to_string()),
                }
            }
        }
        Err(e) => Err(format!("Failed to get bootstrap token: {e:?}")),
    }
}

#[function_component(OnboardingAuth)]
pub fn onboarding_auth() -> Html {
    let webauthn = use_webauthn();
    let name = use_state(String::new);
    let bootstrap_token = use_state(|| None::<String>);
    let is_loading_token = use_state(|| true);
    let token_error = use_state(|| None::<String>);

    // Fetch bootstrap token on mount - check URL params first, then Tauri
    {
        let bootstrap_token = bootstrap_token.clone();
        let is_loading_token = is_loading_token.clone();
        let token_error = token_error.clone();

        use_effect_with((), move |_| {
            // First, check URL query parameters
            if let Some(window) = web_sys::window() {
                if let Ok(location) = window.location().search() {
                    let params = web_sys::UrlSearchParams::new_with_str(&location).ok();
                    if let Some(params) = params {
                        if let Some(token) = params.get("bootstrap_token") {
                            bootstrap_token.set(Some(token));
                            is_loading_token.set(false);
                            return;
                        }
                    }
                }
            }

            // If not in URL and we're in Tauri context, try Tauri API
            if is_tauri() {
                wasm_bindgen_futures::spawn_local(async move {
                    match fetch_bootstrap_token().await {
                        Ok(token) => {
                            bootstrap_token.set(token);
                            is_loading_token.set(false);
                        }
                        Err(e) => {
                            token_error.set(Some(e));
                            is_loading_token.set(false);
                        }
                    }
                });
            } else {
                // Not in Tauri and no token in URL - shouldn't show onboarding
                is_loading_token.set(false);
            }
        });
    }

    // Handle input changes
    let on_name_input = {
        let name = name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            name.set(input.value());
        })
    };

    // Handle registration with bootstrap token
    let on_register = {
        let webauthn = webauthn.clone();
        let name = name.clone();
        let bootstrap_token = bootstrap_token.clone();
        Callback::from(move |_| {
            let name_value = (*name).clone();
            if !name_value.is_empty() {
                webauthn.register(name_value, None, (*bootstrap_token).clone());
            }
        })
    };

    // Loading bootstrap token
    if *is_loading_token {
        return html! {
            <div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-blue-900 to-purple-900">
                <div class="text-center">
                    <LoadingSpinner text={Some("Initializing setup...".to_string())} />
                </div>
            </div>
        };
    }

    // Token error
    if let Some(error) = &*token_error {
        return html! {
            <div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-blue-900 to-purple-900">
                <div class="max-w-md w-full p-8">
                    <div class="bg-red-500/20 border border-red-500/30 rounded-lg p-6 text-center">
                        <h2 class="text-xl font-bold text-white mb-2">{"Setup Error"}</h2>
                        <p class="text-red-200">{error}</p>
                        <p class="text-white/70 mt-4 text-sm">
                            {"Please restart the application and try again."}
                        </p>
                    </div>
                </div>
            </div>
        };
    }

    // No bootstrap token available
    if bootstrap_token.is_none() {
        return html! {
            <div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-blue-900 to-purple-900">
                <div class="max-w-md w-full p-8">
                    <div class="bg-yellow-500/20 border border-yellow-500/30 rounded-lg p-6 text-center">
                        <h2 class="text-xl font-bold text-white mb-2">{"Already Configured"}</h2>
                        <p class="text-yellow-200">
                            {"This instance has already been configured. "}
                            {"Please use your existing credentials to log in."}
                        </p>
                    </div>
                </div>
            </div>
        };
    }

    // Main onboarding UI
    html! {
        <div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-blue-900 to-purple-900">
            <div class="max-w-md w-full p-8">
                <div class="backdrop-blur-lg bg-white/10 rounded-2xl shadow-2xl p-8 border border-white/20">
                    <div class="text-center mb-8">
                        <h1 class="text-3xl font-bold text-white mb-2">{"Welcome to Gate"}</h1>
                        <p class="text-white/70">{"Let's set up your first admin account"}</p>
                    </div>

                    {match webauthn.state() {
                        WebAuthnState::Processing => html! {
                            <div class="text-center">
                                <LoadingSpinner text={Some("Creating your account...".to_string())} />
                                <p class="text-sm text-white/70 mt-4">
                                    {"Use your device's biometrics or security key"}
                                </p>
                            </div>
                        },
                        WebAuthnState::Error(error) => html! {
                            <div class="space-y-4">
                                <div class="bg-red-500/20 border border-red-500/30 rounded-lg p-4 text-center">
                                    <p class="text-red-200 text-sm">{error}</p>
                                </div>
                                {registration_form(&name, &on_name_input, &on_register)}
                            </div>
                        },
                        WebAuthnState::Idle => html! {
                            {registration_form(&name, &on_name_input, &on_register)}
                        }
                    }}

                    <div class="mt-6 text-center">
                        <p class="text-white/50 text-xs">
                            {"This is a one-time setup. Your credentials will be securely stored."}
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn registration_form(
    name: &UseStateHandle<String>,
    on_name_input: &Callback<InputEvent>,
    on_register: &Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="space-y-4">
            <div>
                <label class="block text-white/80 text-sm font-medium mb-2">
                    {"Your Name"}
                </label>
                <input
                    type="text"
                    class="w-full px-4 py-3 bg-white/10 border border-white/20 rounded-lg text-white placeholder-white/50 focus:outline-none focus:border-blue-400 focus:bg-white/20 transition-all"
                    placeholder="Enter your name"
                    value={(**name).clone()}
                    oninput={on_name_input}
                />
                <p class="text-white/50 text-xs mt-2">
                    {"This will be displayed when you log in"}
                </p>
            </div>

            <button
                class="w-full px-4 py-3 bg-gradient-to-r from-blue-500 to-purple-600 hover:from-blue-600 hover:to-purple-700 text-white rounded-lg font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                onclick={on_register}
                disabled={(**name).is_empty()}
            >
                {"Create Admin Account"}
            </button>
        </div>
    }
}
