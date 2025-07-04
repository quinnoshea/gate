use crate::components::DaemonStatusComponent;
use gloo_utils::window;
use wasm_bindgen::JsCast;
use yew::prelude::*;

#[function_component(TauriApp)]
pub fn tauri_app() -> Html {
    let is_dark = use_state(|| {
        window()
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .map(|m| m.matches())
            .unwrap_or(true)
    });

    use_effect_with((), {
        let is_dark = is_dark.clone();
        move |_| {
            let is_dark = is_dark.clone();
            if let Ok(Some(media_query)) = window().match_media("(prefers-color-scheme: dark)") {
                let listener = gloo_events::EventListener::new(&media_query, "change", move |e| {
                    if let Some(event) = e.dyn_ref::<web_sys::MediaQueryListEvent>() {
                        is_dark.set(event.matches());
                    }
                });
                // Keep the listener alive
                listener.forget();
            }
        }
    });

    html! {
        <div class={classes!(
            "h-screen", "w-screen", "font-sans", "transition-colors", "duration-300",
            if *is_dark { "bg-gray-900" } else { "bg-gray-50" }
        )}>
            <div class="h-full w-full p-8 overflow-y-auto">
                <DaemonStatusComponent is_dark={*is_dark} />
            </div>
        </div>
    }
}
