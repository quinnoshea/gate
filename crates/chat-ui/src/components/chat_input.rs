use crate::styles::{
    CARD_BG, ERROR_TEXT, FLEX_CENTER, FLEX_COL, INPUT_COLORS, PRIMARY_BORDER, PRIMARY_TEXT,
    STANDARD_PADDING, TRANSITION_COLORS, combine_styles,
};
use crate::types::{Attachment, MultimodalMessage};
use gloo_timers::callback::Timeout;
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, File, FileReader, HtmlElement, HtmlInputElement, HtmlTextAreaElement, MouseEvent,
    ProgressEvent,
};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ChatInputProps {
    /// Callback for simple text messages (backward compatibility)
    #[prop_or_default]
    pub on_send: Option<Callback<String>>,
    /// Callback for multimodal messages (text + attachments)
    #[prop_or_default]
    pub on_send_multimodal: Option<Callback<MultimodalMessage>>,
    #[prop_or_default]
    pub placeholder: Option<String>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub allow_images: bool,
    #[prop_or_default]
    pub allow_files: bool,
    /// Maximum file size in bytes (default: 10MB)
    #[prop_or(10 * 1024 * 1024)]
    pub max_file_size: usize,
}

#[hook]
fn use_auto_resize_textarea() -> (NodeRef, Callback<()>) {
    let text_area_ref = use_node_ref();

    let resize_textarea = {
        let text_area_ref = text_area_ref.clone();
        Callback::from(move |_| {
            if let Some(text_area) = text_area_ref.cast::<HtmlTextAreaElement>()
                && let Some(element) = text_area.dyn_ref::<HtmlElement>()
            {
                let _ = element.style().set_property("height", "auto");
                let _ = element.style().set_property("overflow-y", "hidden");
                let scroll_height = text_area.scroll_height();
                let new_height = scroll_height.min(200);
                let _ = element
                    .style()
                    .set_property("height", &format!("{}px", new_height));
                if new_height >= 200 {
                    let _ = element.style().set_property("overflow-y", "auto");
                }
            }
        })
    };

    (text_area_ref, resize_textarea)
}

fn clear_input_and_reset(input_value: &UseStateHandle<String>, text_area_ref: &NodeRef) {
    input_value.set(String::new());
    if let Some(text_area) = text_area_ref.cast::<HtmlTextAreaElement>() {
        text_area.set_value("");
        if let Some(element) = text_area.dyn_ref::<HtmlElement>() {
            let _ = element.style().set_property("height", "auto");
        }
    }
}

async fn read_file(file: File) -> Result<Vec<u8>, String> {
    let file_reader = FileReader::new().map_err(|_| "Failed to create FileReader")?;

    let (promise, resolve, reject) = {
        let mut resolve = None;
        let mut reject = None;
        let promise = js_sys::Promise::new(&mut |res, rej| {
            resolve = Some(res);
            reject = Some(rej);
        });
        (promise, resolve.unwrap(), reject.unwrap())
    };

    {
        let reader_clone = file_reader.clone();
        let onload = Closure::<dyn FnMut(_)>::new(move |_: ProgressEvent| {
            if let Ok(result) = reader_clone.result() {
                let _ = resolve.call1(&wasm_bindgen::JsValue::NULL, &result);
            }
        });
        file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        let onerror = Closure::<dyn FnMut(_)>::new(move |_: ProgressEvent| {
            let _ = reject.call1(
                &wasm_bindgen::JsValue::NULL,
                &wasm_bindgen::JsValue::from_str("Failed to read file"),
            );
        });
        file_reader.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    }

    file_reader
        .read_as_array_buffer(&file)
        .map_err(|_| "Failed to read file")?;

    let result = JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to read file")?;
    let array_buffer = result
        .dyn_into::<js_sys::ArrayBuffer>()
        .map_err(|_| "Invalid ArrayBuffer")?;
    let array = js_sys::Uint8Array::new(&array_buffer);
    Ok(array.to_vec())
}

#[function_component(ChatInput)]
pub fn chat_input(props: &ChatInputProps) -> Html {
    let input_value = use_state(String::new);
    let attachments = use_state(Vec::<Attachment>::new);
    let error_message = use_state(|| Option::<String>::None);
    let is_loading = use_state(|| false);
    let file_input_ref = use_node_ref();
    let (text_area_ref, resize_textarea) = use_auto_resize_textarea();

    let _clear_error = {
        let error_message = error_message.clone();
        Callback::from(move |_: ()| {
            error_message.set(None);
        })
    };

    let handle_input = {
        let input_value = input_value.clone();
        let text_area_ref = text_area_ref.clone();
        let resize_textarea = resize_textarea.clone();
        Callback::from(move |_| {
            if let Some(text_area) = text_area_ref.cast::<HtmlTextAreaElement>() {
                input_value.set(text_area.value());
                resize_textarea.emit(());
            }
        })
    };

    let handle_file_select = {
        let attachments = attachments.clone();
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let max_file_size = props.max_file_size;
        let file_input_ref = file_input_ref.clone();

        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<HtmlInputElement>();
            if let Some(files) = input.files() {
                let files_len = files.length();
                if files_len > 0 {
                    is_loading.set(true);
                    error_message.set(None);

                    let attachments = attachments.clone();
                    let error_message = error_message.clone();
                    let is_loading = is_loading.clone();
                    let file_input_ref = file_input_ref.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        let mut new_attachments = (*attachments).clone();
                        let mut has_error = false;

                        for i in 0..files_len {
                            if let Some(file) = files.item(i) {
                                let size = file.size() as usize;
                                if size > max_file_size {
                                    error_message.set(Some(format!(
                                        "{} is too large. Maximum size is {}",
                                        file.name(),
                                        format_file_size(max_file_size)
                                    )));
                                    has_error = true;
                                    break;
                                }

                                match read_file(file.clone()).await {
                                    Ok(data) => {
                                        new_attachments.push(Attachment {
                                            name: file.name(),
                                            mime_type: file.type_(),
                                            data,
                                            size,
                                        });
                                    }
                                    Err(err) => {
                                        error_message.set(Some(err));
                                        has_error = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if !has_error {
                            attachments.set(new_attachments);
                        }
                        is_loading.set(false);

                        // Clear the file input
                        if let Some(input) = file_input_ref.cast::<HtmlInputElement>() {
                            input.set_value("");
                        }
                    });
                }
            }
        })
    };

    let remove_attachment = {
        let attachments = attachments.clone();
        Callback::from(move |index: usize| {
            let mut new_attachments = (*attachments).clone();
            new_attachments.remove(index);
            attachments.set(new_attachments);
        })
    };

    let send_message = {
        let input_value = input_value.clone();
        let attachments = attachments.clone();
        let on_send = props.on_send.clone();
        let on_send_multimodal = props.on_send_multimodal.clone();
        let text_area_ref = text_area_ref.clone();

        Callback::from(move |_| {
            let text = (*input_value).clone();
            let has_text = !text.trim().is_empty();
            let has_attachments = !attachments.is_empty();

            if has_text || has_attachments {
                if has_attachments || on_send_multimodal.is_some() {
                    // Send as multimodal message
                    if let Some(callback) = &on_send_multimodal {
                        let message = MultimodalMessage::with_attachments(
                            if has_text { Some(text) } else { None },
                            (*attachments).clone(),
                        );
                        callback.emit(message);
                    } else if has_text && on_send.is_some() {
                        // Fallback to text-only if no multimodal callback
                        on_send.as_ref().unwrap().emit(text);
                    }
                } else if has_text && on_send.is_some() {
                    // Send as text-only message
                    on_send.as_ref().unwrap().emit(text);
                }

                // Clear everything
                clear_input_and_reset(&input_value, &text_area_ref);
                attachments.set(Vec::new());
            }
        })
    };

    let handle_keydown = {
        let send_message = send_message.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && !e.shift_key() {
                e.prevent_default();
                send_message.emit(());
            }
        })
    };

    let handle_attachment_click = {
        let file_input_ref = file_input_ref.clone();
        Callback::from(move |_| {
            if let Some(input) = file_input_ref.cast::<HtmlInputElement>() {
                input.click();
            }
        })
    };

    // Auto-clear error after 5 seconds
    {
        let error_message = error_message.clone();
        use_effect_with((*error_message).clone(), move |err| {
            if err.is_some() {
                let error_message = error_message.clone();
                let timeout = Timeout::new(5000, move || {
                    error_message.set(None);
                });
                timeout.forget();
            }
            || ()
        });
    }

    let is_empty = input_value.trim().is_empty() && attachments.is_empty();
    let placeholder = props
        .placeholder
        .clone()
        .unwrap_or_else(|| "Type a message...".to_string());

    let show_attachments = props.allow_images || props.allow_files;
    let accept = match (props.allow_images, props.allow_files) {
        (true, true) => "*/*",
        (true, false) => "image/*",
        (false, true) => "*/*",
        (false, false) => "",
    };

    html! {
        <div class={FLEX_COL}>
            // Error message
            if let Some(error) = &*error_message {
                <div class={combine_styles(&["mx-4 mb-2 p-2 rounded-md bg-red-50 dark:bg-red-900/20", ERROR_TEXT, "text-sm"])}>
                    {error}
                </div>
            }

            // Attachment previews
            if !attachments.is_empty() {
                <div class="flex flex-wrap gap-2 px-4 pb-2">
                    {for attachments.iter().enumerate().map(|(i, attachment)| {
                        html! {
                            <AttachmentPreview
                                attachment={attachment.clone()}
                                on_remove={
                                    let remove_attachment = remove_attachment.clone();
                                    Callback::from(move |_: MouseEvent| remove_attachment.emit(i))
                                }
                            />
                        }
                    })}
                </div>
            }

            // Input area
            <div class={combine_styles(&["flex items-end gap-3", STANDARD_PADDING, CARD_BG, "border-t", PRIMARY_BORDER])}>
                // Hidden file input
                if show_attachments {
                    <input
                        ref={file_input_ref}
                        type="file"
                        accept={accept}
                        multiple={true}
                        onchange={handle_file_select}
                        class="hidden"
                    />
                }

                // Attachment button
                if show_attachments {
                    <button
                        onclick={handle_attachment_click}
                        disabled={props.disabled || *is_loading}
                        class={combine_styles(&[
                            FLEX_CENTER, "justify-center w-10 h-10 p-0 rounded-lg",
                            "text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200",
                            "hover:bg-gray-100 dark:hover:bg-gray-700",
                            TRANSITION_COLORS,
                            "disabled:opacity-50 disabled:cursor-not-allowed"
                        ])}
                        title="Attach files"
                    >
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
                        </svg>
                    </button>
                }

                // Text input
                <div class="flex-1 relative">
                    <textarea
                        ref={text_area_ref}
                        class={combine_styles(&["w-full min-h-[40px] max-h-[200px] px-3 py-2 border", INPUT_COLORS, "rounded-lg text-sm leading-relaxed resize-none outline-none overflow-hidden", TRANSITION_COLORS, CARD_BG, PRIMARY_TEXT, "focus:border-gray-400 focus:ring-1 focus:ring-gray-400 dark:focus:border-gray-500 dark:focus:ring-gray-500 disabled:bg-gray-100 disabled:cursor-not-allowed placeholder:text-gray-400 dark:placeholder:text-gray-600"])}
                        placeholder={placeholder}
                        value={(*input_value).clone()}
                        oninput={handle_input}
                        onkeydown={handle_keydown}
                        disabled={props.disabled || *is_loading}
                        rows="1"
                    />
                </div>

                // Send button
                <SendButton
                    onclick={send_message}
                    disabled={is_empty || props.disabled || *is_loading}
                />
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AttachmentPreviewProps {
    pub attachment: Attachment,
    pub on_remove: Callback<MouseEvent>,
}

#[function_component(AttachmentPreview)]
fn attachment_preview(props: &AttachmentPreviewProps) -> Html {
    let attachment = &props.attachment;

    html! {
        <div class="relative group">
            if attachment.is_image() {
                // Image preview
                <div class="relative w-20 h-20 rounded-lg overflow-hidden bg-gray-100 dark:bg-gray-700">
                    <img
                        src={attachment.to_data_url()}
                        alt={attachment.name.clone()}
                        class="w-full h-full object-cover"
                    />
                    <div class="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                        <button
                            onclick={props.on_remove.clone()}
                            class="p-1 bg-red-500 text-white rounded-full hover:bg-red-600"
                            title="Remove attachment"
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>
                </div>
            } else {
                // File preview
                <div class="flex items-center gap-2 px-3 py-2 bg-gray-100 dark:bg-gray-700 rounded-lg group-hover:bg-gray-200 dark:group-hover:bg-gray-600 transition-colors">
                    <svg class="w-5 h-5 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                    </svg>
                    <div class="flex flex-col">
                        <span class="text-sm font-medium text-gray-700 dark:text-gray-300 truncate max-w-[100px]">
                            {&attachment.name}
                        </span>
                        <span class="text-xs text-gray-500 dark:text-gray-400">
                            {attachment.size_string()}
                        </span>
                    </div>
                    <button
                        onclick={props.on_remove.clone()}
                        class="ml-2 p-1 text-gray-500 hover:text-red-500 dark:text-gray-400 dark:hover:text-red-400"
                        title="Remove attachment"
                    >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct SendButtonProps {
    pub onclick: Callback<()>,
    pub disabled: bool,
}

#[function_component(SendButton)]
fn send_button(props: &SendButtonProps) -> Html {
    let handle_click = {
        let onclick = props.onclick.clone();
        Callback::from(move |_| onclick.emit(()))
    };

    html! {
        <button
            class={combine_styles(&[FLEX_CENTER, "justify-center w-[42px] h-[42px] p-0 rounded-lg bg-blue-500 dark:bg-blue-600 text-white cursor-pointer transition-all duration-200 flex-shrink-0 hover:bg-blue-600 dark:hover:bg-blue-700 disabled:bg-gray-200 disabled:cursor-not-allowed disabled:text-gray-400"])}
            onclick={handle_click}
            disabled={props.disabled}
            title="Send message"
            aria-label="Send message"
        >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                <path d="M22 2L11 13" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                <path d="M22 2L15 22L11 13L2 9L22 2Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
        </button>
    }
}

fn format_file_size(size: usize) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}
