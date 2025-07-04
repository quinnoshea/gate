//! Theme context definition

use serde::{Deserialize, Serialize};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub fn toggle(&self) -> Self {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ThemeContext {
    pub theme: Theme,
}

pub enum ThemeAction {
    Set(Theme),
    Toggle,
}

impl Reducible for ThemeContext {
    type Action = ThemeAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            ThemeAction::Set(theme) => {
                // Save to localStorage
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage
                            .set_item("theme", &serde_json::to_string(&theme).unwrap_or_default());
                    }
                }

                // Update document class
                update_document_theme(theme);

                Rc::new(Self { theme })
            }
            ThemeAction::Toggle => {
                let new_theme = self.theme.toggle();

                // Save to localStorage
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.set_item(
                            "theme",
                            &serde_json::to_string(&new_theme).unwrap_or_default(),
                        );
                    }
                }

                // Update document class
                update_document_theme(new_theme);

                Rc::new(Self { theme: new_theme })
            }
        }
    }
}

pub fn update_document_theme(theme: Theme) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(element) = document.document_element() {
                if let Ok(html_element) = element.dyn_into::<web_sys::HtmlElement>() {
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
            }
        }
    }
}
