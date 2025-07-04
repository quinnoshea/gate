//! Theme provider component

use super::context::{Theme, ThemeAction, ThemeContext};
use wasm_bindgen::JsCast;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct ThemeProviderProps {
    pub children: Children,
}

#[function_component(ThemeProvider)]
pub fn theme_provider(props: &ThemeProviderProps) -> Html {
    let theme = use_reducer(|| {
        // Try to load theme from localStorage
        let saved_theme = if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(theme_str)) = storage.get_item("theme") {
                    serde_json::from_str(&theme_str).ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Use saved theme or default
        let theme = saved_theme.unwrap_or_default();

        // Apply theme to document
        if let Some(window) = web_sys::window()
            && let Some(document) = window.document()
            && let Some(element) = document.document_element()
            && let Ok(html_element) = element.dyn_into::<web_sys::HtmlElement>()
        {
            let class_list = html_element.class_list();
            match theme {
                Theme::Dark => {
                    let _ = class_list.add_1("dark");
                }
                Theme::Light => {
                    let _ = class_list.remove_1("dark");
                }
            }
        }

        ThemeContext { theme }
    });

    // Set up system theme preference detection
    {
        let theme = theme.clone();
        use_effect_with((), move |_| {
            // Only set system preference if no saved theme
            if let Some(window) = web_sys::window()
                && let Ok(Some(storage)) = window.local_storage()
                && storage.get_item("theme").unwrap_or(None).is_none()
            {
                // Check system preference using JavaScript evaluation
                if let Ok(matches) = js_sys::eval(
                    "window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches",
                ) && let Some(dark_mode) = matches.as_bool()
                    && dark_mode
                {
                    theme.dispatch(ThemeAction::Set(Theme::Dark));
                }
            }
            || ()
        });
    }

    html! {
        <ContextProvider<UseReducerHandle<ThemeContext>> context={theme}>
            { props.children.clone() }
        </ContextProvider<UseReducerHandle<ThemeContext>>>
    }
}
