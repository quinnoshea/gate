mod app;
mod auth;
mod auth_guard;
mod client;
mod components;
mod config;
mod hooks;
mod local_app;
mod local_auth;
mod services;
mod theme;

use local_app::LocalApp;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<LocalApp>::new().render();
}
