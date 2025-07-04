use crate::components::Message;
use crate::styles::{FLEX_COL_GAP_4, STANDARD_PADDING};
use crate::types::ChatMessage;
use web_sys::Element;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct MessageListProps {
    pub messages: Vec<ChatMessage>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(MessageList)]
pub fn message_list(props: &MessageListProps) -> Html {
    let MessageListProps { messages, class } = props;

    let container_ref = use_node_ref();

    // Auto-scroll to bottom when new messages arrive
    use_effect_with(messages.len(), {
        let container_ref = container_ref.clone();
        move |_| {
            if let Some(element) = container_ref.cast::<Element>() {
                element.set_scroll_top(element.scroll_height());
            }
        }
    });

    html! {
        <div ref={container_ref} class={classes!(STANDARD_PADDING, FLEX_COL_GAP_4, class.clone())}>
            {for messages.iter().enumerate().map(|(index, message)| {
                html! {
                    <Message
                        key={index}
                        message={message.clone()}
                        is_last={index == messages.len() - 1}
                    />
                }
            })}
        </div>
    }
}
