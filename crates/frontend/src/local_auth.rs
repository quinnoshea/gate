use crate::components::LoadingSpinner;
use crate::hooks::{WebAuthnState, use_webauthn};
use yew::prelude::*;

#[function_component(LocalAuth)]
pub fn local_auth() -> Html {
    let webauthn = use_webauthn();

    let name = use_state(String::new);
    let has_tried_auto_auth = use_state(|| false);

    // Handle input changes
    let on_name_input = {
        let name = name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            name.set(input.value());
        })
    };

    // Auto-trigger authentication on mount (only once)
    {
        let webauthn = webauthn.clone();
        let has_tried_auto_auth = has_tried_auto_auth.clone();

        use_effect_with((), move |_| {
            if !*has_tried_auto_auth {
                has_tried_auto_auth.set(true);
                webauthn.authenticate();
            }
        });
    }

    // Handle registration
    let on_register = {
        let webauthn = webauthn.clone();
        let name = name.clone();
        Callback::from(move |_| {
            let name_value = (*name).clone();
            if !name_value.is_empty() {
                webauthn.register(name_value, None, None);
            }
        })
    };

    // Handle login
    let on_login = {
        let webauthn = webauthn.clone();
        Callback::from(move |_| {
            webauthn.authenticate();
        })
    };

    // Clear error
    let on_clear_error = {
        let webauthn = webauthn.clone();
        Callback::from(move |_| {
            webauthn.clear_error();
        })
    };

    // Handle WebAuthn state
    match webauthn.state() {
        WebAuthnState::Processing => {
            html! {
                <div class="text-center">
                    <LoadingSpinner text={Some("Authenticating...".to_string())} />
                    <p class="text-sm text-white/70 mt-4">
                        {"Use your device's biometrics or security key"}
                    </p>
                </div>
            }
        }
        WebAuthnState::Error(error) => {
            html! {
                <div class="space-y-4">
                    <div class="bg-red-500/20 border border-red-500/30 rounded-lg p-4 text-center">
                        <p class="text-red-200 text-sm">{error}</p>
                    </div>

                    // Show registration form if authentication failed
                    <div class="space-y-4">
                        <h3 class="text-white text-center font-medium">{"Create New Account"}</h3>
                        <input
                            type="text"
                            class="w-full px-4 py-3 bg-white/10 border border-white/20 rounded-lg text-white placeholder-white/50 focus:outline-none focus:border-blue-400 focus:bg-white/20 transition-all"
                            placeholder="Enter your name"
                            value={(*name).clone()}
                            oninput={on_name_input}
                        />
                        <div class="flex gap-3">
                            <button
                                class="flex-1 px-4 py-3 bg-gradient-to-r from-blue-500 to-purple-600 hover:from-blue-600 hover:to-purple-700 text-white rounded-lg font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                                onclick={on_register}
                                disabled={(*name).is_empty()}
                            >
                                {"Register"}
                            </button>
                            <button
                                class="flex-1 px-4 py-3 bg-white/10 hover:bg-white/20 text-white rounded-lg font-medium transition-all border border-white/20"
                                onclick={on_clear_error}
                            >
                                {"Try Again"}
                            </button>
                        </div>
                    </div>
                </div>
            }
        }
        WebAuthnState::Idle => {
            html! {
                <div class="space-y-4">
                    <p class="text-white/70 text-center text-sm">
                        {"No existing credentials found"}
                    </p>

                    <input
                        type="text"
                        class="w-full px-4 py-3 bg-white/10 border border-white/20 rounded-lg text-white placeholder-white/50 focus:outline-none focus:border-blue-400 focus:bg-white/20 transition-all"
                        placeholder="Enter your name"
                        value={(*name).clone()}
                        oninput={on_name_input}
                    />

                    <div class="flex gap-3">
                        <button
                            class="flex-1 px-4 py-3 bg-gradient-to-r from-blue-500 to-purple-600 hover:from-blue-600 hover:to-purple-700 text-white rounded-lg font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                            onclick={on_register}
                            disabled={(*name).is_empty()}
                        >
                            {"Register New Device"}
                        </button>
                        <button
                            class="flex-1 px-4 py-3 bg-white/10 hover:bg-white/20 text-white rounded-lg font-medium transition-all border border-white/20"
                            onclick={on_login}
                        >
                            {"Try Existing"}
                        </button>
                    </div>
                </div>
            }
        }
    }
}
