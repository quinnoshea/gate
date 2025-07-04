//! Loading spinner component

use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct SpinnerProps {
    #[prop_or_default]
    pub text: Option<String>,
}

#[function_component(LoadingSpinner)]
pub fn loading_spinner(props: &SpinnerProps) -> Html {
    html! {
        <div class="text-center p-10">
            <div class="w-10 h-10 border-4 border-gray-200 dark:border-gray-700 border-t-blue-500 dark:border-t-blue-400 rounded-full animate-spin mx-auto mb-5"></div>
            if let Some(text) = &props.text {
                <p class="text-gray-600 dark:text-gray-400 text-sm m-0">{text}</p>
            }
        </div>
    }
}
