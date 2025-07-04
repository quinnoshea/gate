use crate::components::{ChatInput, MessageList, StreamingIndicator};
use crate::styles::{
    CARD_BG, CONTAINER_BG, FLEX_BETWEEN, FLEX_CENTER_GAP_2, FLEX_COL, HEADER_PADDING,
    PRIMARY_BORDER, ROUNDED_STANDARD, SECONDARY_TEXT, TERTIARY_TEXT, combine_styles,
};
use crate::types::ChatResponse;
use crate::types::MultimodalMessage;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct ChatContainerProps {
    pub chat_response: ChatResponse,
    #[prop_or_default]
    pub show_metadata: bool,
    #[prop_or_default]
    pub show_input: bool,
    #[prop_or_default]
    pub on_send_message: Option<Callback<String>>,
    #[prop_or_default]
    pub on_send_multimodal: Option<Callback<MultimodalMessage>>,
    #[prop_or_default]
    pub input_disabled: bool,
    #[prop_or_default]
    pub allow_images: bool,
    #[prop_or_default]
    pub allow_files: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(ChatContainer)]
pub fn chat_container(props: &ChatContainerProps) -> Html {
    let ChatContainerProps {
        chat_response,
        show_metadata,
        show_input,
        on_send_message,
        on_send_multimodal,
        input_disabled,
        allow_images,
        allow_files,
        class,
    } = props;

    let is_streaming = chat_response
        .messages
        .last()
        .map(|m| m.is_streaming())
        .unwrap_or(false);

    html! {
        <div class={classes!(FLEX_COL, "h-full", CONTAINER_BG, ROUNDED_STANDARD, "overflow-hidden", class.clone())}>
            if *show_metadata {
                <div class={combine_styles(&[CARD_BG, "border-b", PRIMARY_BORDER, HEADER_PADDING, FLEX_BETWEEN, "flex-shrink-0"])}>
                    <div class={combine_styles(&[FLEX_CENTER_GAP_2, "gap-3"])}>
                        <span class={combine_styles(&["font-semibold", TERTIARY_TEXT, "text-sm"])}>{format!("{:?}", chat_response.provider)}</span>
                        <span class={combine_styles(&[SECONDARY_TEXT, "text-sm"])}>{&chat_response.model}</span>
                    </div>
                    if let Some(usage) = &chat_response.usage {
                        <div class={combine_styles(&["flex gap-4 text-xs", TERTIARY_TEXT])}>
                            if let Some(prompt) = usage.prompt_tokens {
                                <span class="flex items-center gap-1">{format!("Prompt: {} tokens", prompt)}</span>
                            }
                            if let Some(completion) = usage.completion_tokens {
                                <span class="flex items-center gap-1">{format!("Completion: {} tokens", completion)}</span>
                            }
                        </div>
                    }
                </div>
            }

            <div class="flex-1 overflow-y-auto">
                <MessageList messages={chat_response.messages.clone()} />
            </div>

            if is_streaming {
                <StreamingIndicator />
            }

            if *show_input {
                if on_send_message.is_some() || on_send_multimodal.is_some() {
                    <div class="flex-shrink-0">
                        <ChatInput
                            on_send={on_send_message.clone()}
                            on_send_multimodal={on_send_multimodal.clone()}
                            disabled={*input_disabled || is_streaming}
                            allow_images={*allow_images}
                            allow_files={*allow_files}
                        />
                    </div>
                }
            }
        </div>
    }
}
