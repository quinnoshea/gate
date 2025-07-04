use crate::auth::{Auth, AuthAction, AuthProvider, use_auth, use_is_authenticated};
use crate::components::{ChatDemo, LiveChat, ThemeToggle};
use crate::theme::ThemeProvider;
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <ThemeProvider>
            <AuthProvider>
                <AppContent />
            </AuthProvider>
        </ThemeProvider>
    }
}

#[function_component(AppContent)]
fn app_content() -> Html {
    let auth = use_auth();
    let is_authenticated = use_is_authenticated();
    let show_chat_demo = use_state(|| false);
    let show_live_chat = use_state(|| false);
    let show_auth = use_state(|| false);

    let on_logout = {
        let auth = auth.clone();
        Callback::from(move |_| {
            auth.dispatch(AuthAction::Logout);
        })
    };

    let toggle_chat_demo = {
        let show_chat_demo = show_chat_demo.clone();
        let show_live_chat = show_live_chat.clone();
        let show_auth = show_auth.clone();
        Callback::from(move |_| {
            show_chat_demo.set(!*show_chat_demo);
            show_live_chat.set(false);
            show_auth.set(false);
        })
    };

    let toggle_live_chat = {
        let show_live_chat = show_live_chat.clone();
        let show_chat_demo = show_chat_demo.clone();
        let show_auth = show_auth.clone();
        Callback::from(move |_| {
            if is_authenticated {
                show_live_chat.set(!*show_live_chat);
                show_chat_demo.set(false);
                show_auth.set(false);
            }
        })
    };

    let toggle_auth = {
        let show_auth = show_auth.clone();
        let show_chat_demo = show_chat_demo.clone();
        let show_live_chat = show_live_chat.clone();
        Callback::from(move |_| {
            show_auth.set(!*show_auth);
            show_chat_demo.set(false);
            show_live_chat.set(false);
        })
    };

    // Auto-redirect to live chat after successful auth
    {
        let show_auth = show_auth.clone();
        let show_live_chat = show_live_chat.clone();
        use_effect_with(is_authenticated, move |is_auth| {
            if *is_auth && *show_auth {
                show_auth.set(false);
                show_live_chat.set(true);
            }
        });
    }

    if *show_chat_demo {
        return html! {
            <div class="h-screen flex flex-col bg-white dark:bg-gray-900">
                <div class="p-4 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
                    <button onclick={toggle_chat_demo} class="flex items-center gap-2 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 transition-colors">
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path>
                        </svg>
                        {"Back to Home"}
                    </button>
                    <ThemeToggle />
                </div>
                <ChatDemo />
            </div>
        };
    }

    if *show_live_chat && is_authenticated {
        return html! {
            <div class="h-screen flex flex-col bg-white dark:bg-gray-900">
                <div class="p-4 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
                    <button onclick={toggle_live_chat} class="flex items-center gap-2 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 transition-colors">
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path>
                        </svg>
                        {"Back to Home"}
                    </button>
                    <ThemeToggle />
                </div>
                <LiveChat />
            </div>
        };
    }

    if *show_auth {
        return html! {
            <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 dark:from-gray-900 dark:to-gray-800 flex items-center justify-center px-4">
                <div class="max-w-md w-full">
                    <div class="text-center mb-8">
                        <h1 class="text-3xl font-bold bg-gradient-to-r from-blue-600 to-purple-600 bg-clip-text text-transparent">
                            {"Gate"}
                        </h1>
                        <p class="mt-2 text-gray-600 dark:text-gray-400">{"Sign in to access Live Chat"}</p>
                    </div>
                    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-lg p-8">
                        <Auth />
                        <div class="mt-6 text-center">
                            <button
                                onclick={toggle_auth}
                                class="text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100"
                            >
                                {"← Back to Home"}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        };
    }

    html! {
        <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 dark:from-gray-900 dark:to-gray-800">
            // Navigation
            <nav class="bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm border-b border-gray-200 dark:border-gray-700">
                <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                    <div class="flex justify-between h-16 items-center">
                        <div class="flex items-center">
                            <h1 class="text-2xl font-bold bg-gradient-to-r from-blue-600 to-purple-600 bg-clip-text text-transparent">
                                {"Gate"}
                            </h1>
                            <span class="ml-3 text-sm text-gray-500 dark:text-gray-400">{"AI Gateway"}</span>
                        </div>
                        <div class="flex items-center gap-4">
                            if is_authenticated {
                                if let Some(auth_state) = &auth.auth_state {
                                    <span class="text-sm text-gray-600 dark:text-gray-400">
                                        {format!("Welcome, {}", auth_state.name)}
                                    </span>
                                    <button onclick={on_logout} class="text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">
                                        {"Sign Out"}
                                    </button>
                                }
                            } else {
                                <button
                                    onclick={toggle_auth.clone()}
                                    class="px-4 py-2 text-sm font-medium text-white bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 rounded-md transition-all"
                                >
                                    {"Sign In"}
                                </button>
                            }
                            <ThemeToggle />
                        </div>
                    </div>
                </div>
            </nav>

            // Hero Section
            <div class="relative overflow-hidden">
                <div class="max-w-7xl mx-auto">
                    <div class="relative z-10 pb-8 sm:pb-16 md:pb-20 lg:pb-28 xl:pb-32">
                        <main class="mt-10 mx-auto max-w-7xl px-4 sm:mt-12 sm:px-6 md:mt-16 lg:mt-20 lg:px-8 xl:mt-28">
                            <div class="text-center">
                                <h1 class="text-4xl tracking-tight font-extrabold text-gray-900 dark:text-white sm:text-5xl md:text-6xl">
                                    <span class="block">{"Unified AI Gateway"}</span>
                                    <span class="block text-transparent bg-clip-text bg-gradient-to-r from-blue-600 to-purple-600 mt-2">
                                        {"for Modern Applications"}
                                    </span>
                                </h1>
                                <p class="mt-3 max-w-md mx-auto text-base text-gray-500 dark:text-gray-400 sm:text-lg md:mt-5 md:text-xl md:max-w-3xl">
                                    {"Gate provides a unified interface to multiple AI providers with built-in authentication, monitoring, and extensibility. Deploy anywhere, integrate once."}
                                </p>
                                <div class="mt-5 max-w-md mx-auto sm:flex sm:justify-center md:mt-8">
                                    <div class="rounded-md shadow">
                                        <button
                                            onclick={if is_authenticated { toggle_live_chat.clone() } else { toggle_auth.clone() }}
                                            class="w-full flex items-center justify-center px-8 py-3 border border-transparent text-base font-medium rounded-md text-white bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 md:py-4 md:text-lg md:px-10 transition-all transform hover:scale-105"
                                        >
                                            {if is_authenticated { "Try Live Chat" } else { "Sign In to Chat" }}
                                            <svg class="ml-2 -mr-1 w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6"></path>
                                            </svg>
                                        </button>
                                    </div>
                                    <div class="mt-3 rounded-md shadow sm:mt-0 sm:ml-3">
                                        <button
                                            onclick={toggle_chat_demo}
                                            class="w-full flex items-center justify-center px-8 py-3 border border-transparent text-base font-medium rounded-md text-gray-700 dark:text-gray-200 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-700 md:py-4 md:text-lg md:px-10 transition-all"
                                        >
                                            {"View Demo"}
                                            <span class="ml-2 text-xs text-gray-500 dark:text-gray-400">{"(No login required)"}</span>
                                        </button>
                                    </div>
                                </div>
                            </div>
                        </main>
                    </div>
                </div>
            </div>

            // Features Section
            <div class="py-12 bg-white dark:bg-gray-800">
                <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                    <div class="lg:text-center">
                        <h2 class="text-base text-blue-600 dark:text-blue-400 font-semibold tracking-wide uppercase">{"Features"}</h2>
                        <p class="mt-2 text-3xl leading-8 font-extrabold tracking-tight text-gray-900 dark:text-white sm:text-4xl">
                            {"Everything you need in an AI Gateway"}
                        </p>
                    </div>

                    <div class="mt-10">
                        <div class="grid grid-cols-1 gap-10 sm:grid-cols-2 lg:grid-cols-3">
                            // Feature 1: Multiple Providers
                            <div class="relative">
                                <div class="flex items-center justify-center h-12 w-12 rounded-md bg-gradient-to-r from-blue-500 to-blue-600 text-white">
                                    <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"></path>
                                    </svg>
                                </div>
                                <div class="mt-5">
                                    <h3 class="text-lg leading-6 font-medium text-gray-900 dark:text-white">{"Multiple Providers"}</h3>
                                    <p class="mt-2 text-base text-gray-500 dark:text-gray-400">
                                        {"Connect to OpenAI, Anthropic, Google, and more through a single unified API. Switch providers without changing your code."}
                                    </p>
                                </div>
                            </div>

                            // Feature 2: Built-in Auth
                            <div class="relative">
                                <div class="flex items-center justify-center h-12 w-12 rounded-md bg-gradient-to-r from-purple-500 to-purple-600 text-white">
                                    <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path>
                                    </svg>
                                </div>
                                <div class="mt-5">
                                    <h3 class="text-lg leading-6 font-medium text-gray-900 dark:text-white">{"Built-in Authentication"}</h3>
                                    <p class="mt-2 text-base text-gray-500 dark:text-gray-400">
                                        {"WebAuthn support, API key management, and JWT tokens. Secure your AI endpoints with modern authentication."}
                                    </p>
                                </div>
                            </div>

                            // Feature 3: Extensible
                            <div class="relative">
                                <div class="flex items-center justify-center h-12 w-12 rounded-md bg-gradient-to-r from-green-500 to-green-600 text-white">
                                    <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"></path>
                                    </svg>
                                </div>
                                <div class="mt-5">
                                    <h3 class="text-lg leading-6 font-medium text-gray-900 dark:text-white">{"Plugin System"}</h3>
                                    <p class="mt-2 text-base text-gray-500 dark:text-gray-400">
                                        {"Extend Gate with custom authentication, billing, rate limiting, and request processing plugins."}
                                    </p>
                                </div>
                            </div>

                            // Feature 4: State Backends
                            <div class="relative">
                                <div class="flex items-center justify-center h-12 w-12 rounded-md bg-gradient-to-r from-yellow-500 to-yellow-600 text-white">
                                    <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4"></path>
                                    </svg>
                                </div>
                                <div class="mt-5">
                                    <h3 class="text-lg leading-6 font-medium text-gray-900 dark:text-white">{"Flexible Storage"}</h3>
                                    <p class="mt-2 text-base text-gray-500 dark:text-gray-400">
                                        {"Use SQLite for development, PostgreSQL for production, or implement your own state backend."}
                                    </p>
                                </div>
                            </div>

                            // Feature 5: Monitoring
                            <div class="relative">
                                <div class="flex items-center justify-center h-12 w-12 rounded-md bg-gradient-to-r from-pink-500 to-pink-600 text-white">
                                    <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"></path>
                                    </svg>
                                </div>
                                <div class="mt-5">
                                    <h3 class="text-lg leading-6 font-medium text-gray-900 dark:text-white">{"Usage Tracking"}</h3>
                                    <p class="mt-2 text-base text-gray-500 dark:text-gray-400">
                                        {"Monitor API usage, track costs, and analyze performance across all your AI providers."}
                                    </p>
                                </div>
                            </div>

                            // Feature 6: Open Source
                            <div class="relative">
                                <div class="flex items-center justify-center h-12 w-12 rounded-md bg-gradient-to-r from-indigo-500 to-indigo-600 text-white">
                                    <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"></path>
                                    </svg>
                                </div>
                                <div class="mt-5">
                                    <h3 class="text-lg leading-6 font-medium text-gray-900 dark:text-white">{"Open Source"}</h3>
                                    <p class="mt-2 text-base text-gray-500 dark:text-gray-400">
                                        {"Self-host with confidence. Full source code available, no vendor lock-in, deploy anywhere."}
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // CTA Section
            <div class="bg-gradient-to-r from-blue-600 to-purple-600">
                <div class="max-w-2xl mx-auto text-center py-16 px-4 sm:py-20 sm:px-6 lg:px-8">
                    <h2 class="text-3xl font-extrabold text-white sm:text-4xl">
                        <span class="block">{"Ready to get started?"}</span>
                    </h2>
                    <p class="mt-4 text-lg leading-6 text-blue-100">
                        {"Experience the power of a unified AI gateway. Try our live chat demo to see Gate in action."}
                    </p>
                    <button
                        onclick={if is_authenticated { toggle_live_chat } else { toggle_auth }}
                        class="mt-8 w-full inline-flex items-center justify-center px-5 py-3 border border-transparent text-base font-medium rounded-md text-blue-600 bg-white hover:bg-blue-50 sm:w-auto transition-all transform hover:scale-105"
                    >
                        {if is_authenticated { "Start Chatting" } else { "Sign In to Start" }}
                        <svg class="ml-2 -mr-1 w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"></path>
                        </svg>
                    </button>
                </div>
            </div>

            // Footer
            <footer class="bg-white dark:bg-gray-900">
                <div class="max-w-7xl mx-auto py-12 px-4 sm:px-6 lg:px-8">
                    <div class="flex justify-center space-x-6">
                        <a href="https://github.com/your-org/gate" class="text-gray-400 hover:text-gray-500">
                            <span class="sr-only">{"GitHub"}</span>
                            <svg class="h-6 w-6" fill="currentColor" viewBox="0 0 24 24">
                                <path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"></path>
                            </svg>
                        </a>
                    </div>
                    <p class="mt-8 text-center text-base text-gray-400">
                        {"© 2024 Gate. Open source AI gateway."}
                    </p>
                </div>
            </footer>
        </div>
    }
}
