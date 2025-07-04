//! Legacy Auth component wrapper

use super::AuthComponent;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct AuthProps {
    /// Optional callback when auth completes (for compatibility)
    #[prop_or_default]
    pub on_auth: Option<Callback<(String, String, String)>>, // user_id, name, token
}

/// Legacy Auth component that wraps the new function component
pub struct Auth;

impl Component for Auth {
    type Message = ();
    type Properties = AuthProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <AuthComponent />
        }
    }
}
