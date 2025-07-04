//! Theme management module

mod context;
mod provider;

pub use context::{Theme, ThemeAction, ThemeContext};
pub use provider::ThemeProvider;

use yew::prelude::*;

/// Hook to access theme context
#[hook]
pub fn use_theme() -> UseReducerHandle<ThemeContext> {
    use_context::<UseReducerHandle<ThemeContext>>()
        .expect("Theme context not found. Make sure to wrap your app with ThemeProvider")
}

/// Hook to get current theme
#[hook]
pub fn use_current_theme() -> Theme {
    let theme_ctx = use_theme();
    theme_ctx.theme
}

/// Hook to get theme toggle callback
#[hook]
pub fn use_theme_toggle() -> Callback<()> {
    let theme_ctx = use_theme();
    Callback::from(move |_| {
        theme_ctx.dispatch(ThemeAction::Toggle);
    })
}
