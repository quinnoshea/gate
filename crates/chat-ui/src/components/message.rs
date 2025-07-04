use crate::components::StreamingText;
use crate::styles::{
    ASSISTANT_BUBBLE_COLORS, DEFAULT_BUBBLE_COLORS, SYSTEM_BUBBLE_COLORS, TOOL_BUBBLE_COLORS,
    USER_BUBBLE_COLORS,
};
use crate::types::ChatMessage;
use crate::utils::markdown::render_markdown;
use serde_json::Value;
use web_sys::MouseEvent;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct MessageProps {
    pub message: ChatMessage,
    #[prop_or_default]
    pub is_last: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Message)]
pub fn message(props: &MessageProps) -> Html {
    let MessageProps {
        message,
        is_last,
        class,
    } = props;

    let expanded_tools = use_state(|| true); // Default to expanded for better visibility

    let role_class = match message.role.as_str() {
        "system" => "system",
        "user" => "user",
        "assistant" => "assistant",
        "tool" | "function" => "tool",
        _ => "unknown",
    };

    let role_label = match message.role.as_str() {
        "system" => "System",
        "user" => "User",
        "assistant" => "Assistant",
        "tool" => "Tool",
        "function" => "Function",
        _ => &message.role,
    };

    let toggle_tools = {
        let expanded_tools = expanded_tools.clone();
        Callback::from(move |_| {
            expanded_tools.set(!*expanded_tools);
        })
    };

    // Extract refusal from metadata
    let refusal = message
        .metadata
        .get("refusal")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Extract finish_reason from metadata
    let finish_reason = message.finish_reason();

    let message_class = match role_class {
        "user" => USER_BUBBLE_COLORS,
        "assistant" => ASSISTANT_BUBBLE_COLORS,
        "system" => SYSTEM_BUBBLE_COLORS,
        "tool" => TOOL_BUBBLE_COLORS,
        _ => DEFAULT_BUBBLE_COLORS,
    };

    html! {
        <div class={classes!("flex", "flex-col", "gap-2", "p-4", "rounded-lg", "shadow", message_class, class.clone())}>
            <div class="flex gap-2 items-center">
                <span class="font-semibold text-sm text-gray-600 dark:text-gray-400">{role_label}</span>
                if message.name.is_some() {
                    <span class="text-xs text-gray-500">{message.name.as_ref().unwrap()}</span>
                }
            </div>

            <div class="leading-relaxed break-words">
                {render_message_content(&message.content, message.is_streaming(), *is_last)}

                if let Some(refusal_text) = refusal {
                    <div class="bg-red-50 dark:bg-red-900 text-red-700 dark:text-red-300 px-3 py-2 rounded mt-2">
                        <span class="font-semibold mr-2">{"Refusal:"}</span>
                        <span>{refusal_text}</span>
                    </div>
                }

                if let Some(tool_calls) = &message.tool_calls {
                    <div class="mt-2">
                        <button class="bg-transparent border-0 cursor-pointer text-sm text-blue-600 dark:text-blue-400 px-2 py-1 flex items-center gap-1 hover:bg-blue-50 dark:hover:bg-blue-900 rounded" onclick={toggle_tools}>
                            <span class="text-xs w-3">{if *expanded_tools { "▼" } else { "▶" }}</span>
                            <span class="text-base">{"🔧"}</span>
                            {format!(" {} tool call{}", tool_calls.len(), if tool_calls.len() == 1 { "" } else { "s" })}
                        </button>

                        if *expanded_tools {
                            <div class="mt-2 pl-5">
                                {for tool_calls.iter().map(|tool_call| {
                                    render_tool_call(tool_call)
                                })}
                            </div>
                        }
                    </div>
                }
            </div>

            if let Some(reason) = finish_reason {
                <div class="text-xs text-gray-500 italic mt-1">
                    <span>{format!("Finish reason: {}", reason)}</span>
                </div>
            }

            // Display any extra metadata that might be interesting
            {render_extra_metadata(&message.metadata)}

        </div>
    }
}

fn render_message_content(content: &Option<Value>, is_streaming: bool, is_last: bool) -> Html {
    match content {
        None => html! { <span class="text-gray-400 dark:text-gray-400 italic">{"(empty)"}</span> },
        Some(Value::String(text)) => {
            if is_streaming && is_last {
                html! {
                    <StreamingText text={text.clone()} streaming={true} initial_delay_ms={20} final_delay_ms={4} />
                }
            } else {
                render_markdown(text)
            }
        }
        Some(Value::Array(parts)) => {
            html! {
                <>
                    {for parts.iter().map(|part| {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            if is_streaming && is_last {
                                html! {
                                    <StreamingText text={text.to_string()} streaming={true} initial_delay_ms={20} final_delay_ms={4} />
                                }
                            } else {
                                render_markdown(text)
                            }
                        } else if let Some(image_url) = part.get("image_url") {
                            // OpenAI format
                            if let Some(url) = image_url.get("url").and_then(|u| u.as_str()) {
                                html! {
                                    <ImageDisplay url={url.to_string()} />
                                }
                            } else {
                                html! {}
                            }
                        } else if part.get("type").and_then(|t| t.as_str()) == Some("image") {
                            // Anthropic format
                            if let Some(source) = part.get("source") {
                                if let Some(data) = source.get("data").and_then(|d| d.as_str()) {
                                    if let Some(media_type) = source.get("media_type").and_then(|m| m.as_str()) {
                                        let url = format!("data:{};base64,{}", media_type, data);
                                        html! {
                                            <ImageDisplay url={url} />
                                        }
                                    } else {
                                        html! {}
                                    }
                                } else {
                                    html! {}
                                }
                            } else {
                                html! {}
                            }
                        } else {
                            // For other content types, show as JSON
                            html! {
                                <pre class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-2 font-mono text-xs overflow-x-auto whitespace-pre-wrap">
                                    {serde_json::to_string_pretty(part).unwrap_or_default()}
                                </pre>
                            }
                        }
                    })}
                </>
            }
        }
        Some(Value::Object(obj)) => {
            // Try to extract text content
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                if is_streaming && is_last {
                    html! {
                        <StreamingText text={text.to_string()} streaming={true} initial_delay_ms={20} final_delay_ms={4} />
                    }
                } else {
                    render_markdown(text)
                }
            } else {
                // Show as formatted JSON
                html! {
                    <pre class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-2 font-mono text-xs overflow-x-auto whitespace-pre-wrap">
                        {serde_json::to_string_pretty(obj).unwrap_or_default()}
                    </pre>
                }
            }
        }
        Some(other) => {
            // For other types, show as JSON
            html! {
                <pre class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-2 font-mono text-xs overflow-x-auto whitespace-pre-wrap">
                    {serde_json::to_string_pretty(other).unwrap_or_default()}
                </pre>
            }
        }
    }
}

fn render_tool_call(tool_call: &Value) -> Html {
    let id = tool_call
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let function_name = tool_call
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let args = tool_call
        .get("function")
        .and_then(|f| f.get("arguments"))
        .and_then(|a| a.as_str())
        .and_then(|args_str| serde_json::from_str::<Value>(args_str).ok())
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| {
            tool_call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                .unwrap_or("")
                .to_string()
        });

    html! {
        <div class="bg-gray-100 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-3 mb-2">
            <div class="font-semibold text-gray-700 dark:text-gray-300 mb-2">
                {format!("{} ({})", function_name, id)}
            </div>
            <pre class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-2 font-mono text-xs overflow-x-auto whitespace-pre-wrap">
                {args}
            </pre>
        </div>
    }
}

fn render_extra_metadata(metadata: &std::collections::HashMap<String, Value>) -> Html {
    // Filter out known fields that are already displayed
    let skip_fields = ["finish_reason", "refusal", "is_streaming"];
    let extra_fields: Vec<_> = metadata
        .iter()
        .filter(|(key, _)| !skip_fields.contains(&key.as_str()))
        .collect();

    if extra_fields.is_empty() {
        return html! {};
    }

    html! {
        <div class="text-xs text-gray-500 dark:text-gray-400 mt-2 p-2 bg-gray-50 dark:bg-gray-800 rounded">
            {for extra_fields.iter().map(|(key, value)| {
                html! {
                    <div class="my-0.5">
                        <span class="font-semibold mr-1">{format!("{}: ", key)}</span>
                        <span>{format_metadata_value(value)}</span>
                    </div>
                }
            })}
        </div>
    }
}

fn format_metadata_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

#[derive(Properties, Clone, PartialEq)]
struct ImageDisplayProps {
    pub url: String,
}

#[function_component(ImageDisplay)]
fn image_display(props: &ImageDisplayProps) -> Html {
    let expanded = use_state(|| false);

    let toggle_expanded = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| {
            expanded.set(!*expanded);
        })
    };

    if *expanded {
        // Full screen overlay
        html! {
            <>
                <div class="fixed inset-0 bg-black/90 z-50 flex items-center justify-center p-4"
                     onclick={toggle_expanded.clone()}>
                    <img
                        src={props.url.clone()}
                        alt="Expanded image"
                        class="max-w-full max-h-full object-contain cursor-zoom-out"
                        onclick={|e: MouseEvent| e.stop_propagation()}
                    />
                    <button
                        class="absolute top-4 right-4 text-white bg-black/50 rounded-full p-2 hover:bg-black/70 transition-colors"
                        onclick={toggle_expanded}
                        title="Close"
                    >
                        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>
                <div class="my-2 max-w-md">
                    <img
                        src={props.url.clone()}
                        alt="Message image"
                        class="max-w-full h-auto rounded-lg shadow-md cursor-zoom-in hover:shadow-lg transition-shadow"
                    />
                </div>
            </>
        }
    } else {
        // Thumbnail view
        html! {
            <div class="my-2 max-w-md">
                <img
                    src={props.url.clone()}
                    alt="Message image"
                    class="max-w-full h-auto rounded-lg shadow-md cursor-zoom-in hover:shadow-lg transition-shadow"
                    onclick={toggle_expanded}
                    title="Click to expand"
                />
            </div>
        }
    }
}
