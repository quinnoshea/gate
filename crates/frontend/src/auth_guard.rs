//! Authentication guard component for protected routes

use crate::auth::{Auth, use_auth};
use yew::prelude::*;

/// RequireAuth component - simple auth guard
#[derive(Properties, PartialEq)]
pub struct RequireAuthProps {
    pub children: Children,
}

/// Simple auth guard that shows Auth component when not authenticated
#[function_component(RequireAuth)]
pub fn require_auth(props: &RequireAuthProps) -> Html {
    let auth = use_auth();

    // Show loading state
    if auth.is_loading {
        return html! {
            <div class="flex flex-col items-center justify-center min-h-screen">
                <div class="w-10 h-10 border-4 border-gray-200 dark:border-gray-700 border-t-blue-500 dark:border-t-blue-400 rounded-full animate-spin mb-4"></div>
                <p class="text-gray-600 dark:text-gray-400">{"Checking authentication..."}</p>
            </div>
        };
    }

    // Show children if authenticated
    if auth.auth_state.is_some() {
        return html! { <>{ props.children.clone() }</> };
    }

    // Show auth component
    html! {
        <div class="min-h-screen bg-gray-50 dark:bg-gray-900 flex items-center justify-center">
            <Auth />
        </div>
    }
}
