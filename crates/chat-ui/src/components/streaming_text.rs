use crate::utils::markdown::render_markdown;
use gloo_timers::callback::{Interval, Timeout};
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct StreamingTextProps {
    pub text: String,
    pub streaming: bool,
    #[prop_or(25)]
    pub initial_delay_ms: u32,
    #[prop_or(5)]
    pub final_delay_ms: u32,
    #[prop_or_default]
    pub class: Classes,
}

#[derive(Clone, PartialEq)]
struct StreamingState {
    current_text: String,
    current_index: usize,
    start_time: f64,
}

enum StreamingAction {
    Reset,
    Advance(String, usize), // text, index
    SetComplete(String),
}

impl Reducible for StreamingState {
    type Action = StreamingAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let now = js_sys::Date::now();
        match action {
            StreamingAction::Reset => Rc::new(StreamingState {
                current_text: String::new(),
                current_index: 0,
                start_time: now,
            }),
            StreamingAction::Advance(text, index) => Rc::new(StreamingState {
                current_text: text,
                current_index: index,
                start_time: self.start_time,
            }),
            StreamingAction::SetComplete(text) => Rc::new(StreamingState {
                current_text: text.clone(),
                current_index: text.len(),
                start_time: self.start_time,
            }),
        }
    }
}

#[function_component(StreamingText)]
pub fn streaming_text(props: &StreamingTextProps) -> Html {
    let StreamingTextProps {
        text,
        streaming,
        initial_delay_ms,
        final_delay_ms,
        class,
    } = props;

    let state = use_reducer(|| StreamingState {
        current_text: String::new(),
        current_index: 0,
        start_time: js_sys::Date::now(),
    });
    let interval_handle = use_state(|| None::<Interval>);

    // Reset when text changes
    use_effect_with(text.clone(), {
        let state = state.clone();
        let interval_handle = interval_handle.clone();
        move |_| {
            interval_handle.set(None); // Clear interval
            state.dispatch(StreamingAction::Reset);
        }
    });

    // Streaming effect
    use_effect_with(
        (text.clone(), *streaming, *initial_delay_ms, *final_delay_ms),
        {
            let state = state.clone();
            let interval_handle = interval_handle.clone();

            move |(text, streaming, initial_delay, final_delay)| {
                // Clone values to avoid lifetime issues
                let text = text.clone();
                let streaming = *streaming;
                let initial_delay = *initial_delay;
                let final_delay = *final_delay;

                // Clear any existing interval
                interval_handle.set(None);

                if !streaming {
                    // If not streaming, show all text immediately
                    state.dispatch(StreamingAction::SetComplete(text));
                    return;
                }

                // Start with empty text
                state.dispatch(StreamingAction::Reset);

                // Create interval for streaming animation
                let current_idx = Rc::new(RefCell::new(0usize));
                let _start_time = js_sys::Date::now();

                let setup_next_interval = Rc::new(RefCell::new(None::<Box<dyn Fn()>>));
                let setup_next_interval_clone = setup_next_interval.clone();

                *setup_next_interval.borrow_mut() = Some(Box::new({
                    let state = state.clone();
                    let interval_handle = interval_handle.clone();
                    let text = text.clone();
                    let current_idx = current_idx.clone();
                    let setup_next_interval = setup_next_interval_clone.clone();

                    move || {
                        let idx = *current_idx.borrow();

                        if idx >= text.len() {
                            // Done streaming, clear interval
                            interval_handle.set(None);
                            return;
                        }

                        // Find next character boundary to avoid splitting UTF-8
                        let mut next_idx = idx + 1;
                        while next_idx < text.len() && !text.is_char_boundary(next_idx) {
                            next_idx += 1;
                        }

                        // Calculate progress and delay with aggressive acceleration
                        let progress = next_idx as f64 / text.len() as f64;
                        let normalized = progress.clamp(0.0, 1.0);
                        // Much more aggressive exponential curve: -8.0 instead of -3.0
                        let factor = 1.0 - (-8.0 * normalized).exp();
                        let delay =
                            initial_delay as f64 - (initial_delay - final_delay) as f64 * factor;
                        let delay = delay.max(final_delay as f64) as u32;

                        // Update displayed text
                        let new_text = text[..next_idx].to_string();

                        // Update both the display state and our local index
                        state.dispatch(StreamingAction::Advance(new_text, next_idx));
                        *current_idx.borrow_mut() = next_idx;

                        // Schedule next update with dynamic delay
                        if next_idx < text.len() {
                            let setup_clone = setup_next_interval.clone();
                            let timeout = Timeout::new(delay, move || {
                                if let Some(ref setup_fn) = *setup_clone.borrow() {
                                    setup_fn();
                                }
                            });
                            timeout.forget();
                        }
                    }
                }));

                // Start the first interval
                if let Some(ref setup_fn) = *setup_next_interval.borrow() {
                    setup_fn();
                }
            }
        },
    );

    // Cleanup interval on unmount
    use_effect_with((), {
        let interval_handle = interval_handle.clone();
        move |_| {
            move || {
                interval_handle.set(None);
            }
        }
    });

    // Simple, clean rendering without character-level spans
    let displayed_text = if *streaming {
        &state.current_text
    } else {
        text
    };

    html! {
        <div class={classes!("relative", "transition-opacity", "duration-150", "ease-out", class.clone())}>
            {render_markdown(displayed_text)}

            if *streaming && state.current_index < text.len() {
                <span class="inline-block ml-0.5 text-gray-600 dark:text-gray-400 align-baseline animate-pulse">{"▋"}</span>
            }
        </div>
    }
}
