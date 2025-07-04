use crate::styles::FLEX_CENTER;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct StreamingIndicatorProps {
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(StreamingIndicator)]
pub fn streaming_indicator(props: &StreamingIndicatorProps) -> Html {
    let StreamingIndicatorProps { class } = props;

    html! {
        <div class={classes!(FLEX_CENTER, "py-2", "px-4", "mx-4", "mb-4", class.clone())}>
            <div class="flex gap-1">
                <span class="w-2 h-2 rounded-full bg-gray-600 dark:bg-gray-400 animate-pulse-dot" style="animation-delay: -0.32s;"></span>
                <span class="w-2 h-2 rounded-full bg-gray-600 dark:bg-gray-400 animate-pulse-dot" style="animation-delay: -0.16s;"></span>
                <span class="w-2 h-2 rounded-full bg-gray-600 dark:bg-gray-400 animate-pulse-dot"></span>
            </div>
        </div>
    }
}
