use crate::components::NodesList;
use gate_frontend_common::{components::ThemeToggle, theme::ThemeProvider};
use yew::prelude::*;

#[function_component(RelayApp)]
pub fn relay_app() -> Html {
    html! {
        <ThemeProvider>
            <RelayAppContent />
        </ThemeProvider>
    }
}

#[function_component(RelayAppContent)]
fn relay_app_content() -> Html {
    html! {
        <div class="h-screen w-screen flex flex-col bg-gray-50 dark:bg-gray-900">
            <header class="flex items-center justify-between p-4 bg-white dark:bg-gray-800 shadow-sm">
                <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{"Gate Relay"}</h1>
                <div class="flex items-center gap-4">
                    <ThemeToggle />
                </div>
            </header>
            <main class="flex-1 p-6 overflow-y-auto">
                <div class="max-w-6xl mx-auto space-y-6">
                    <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
                        <h2 class="text-xl font-semibold mb-4 text-gray-900 dark:text-white">{"TLS Forward Server Status"}</h2>
                        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                            <div class="bg-gray-50 dark:bg-gray-900 rounded p-4">
                                <p class="text-sm text-gray-600 dark:text-gray-400">{"Status"}</p>
                                <p class="text-lg font-semibold text-green-600 dark:text-green-400">{"Online"}</p>
                            </div>
                            <div class="bg-gray-50 dark:bg-gray-900 rounded p-4">
                                <p class="text-sm text-gray-600 dark:text-gray-400">{"Domain Suffix"}</p>
                                <p class="text-lg font-semibold text-gray-900 dark:text-white">{"private.hellas.ai"}</p>
                            </div>
                            <div class="bg-gray-50 dark:bg-gray-900 rounded p-4">
                                <p class="text-sm text-gray-600 dark:text-gray-400">{"HTTPS Proxy"}</p>
                                <p class="text-lg font-semibold text-gray-900 dark:text-white">{"443"}</p>
                            </div>
                        </div>
                    </div>

                    <NodesList />
                </div>
            </main>
        </div>
    }
}
