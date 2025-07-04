mod components;
mod local_app;
mod local_auth;
mod onboarding;
mod services;
mod utils;

use local_app::LocalApp;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<LocalApp>::new().render();
}
