mod app;
mod components;
mod tauri_api;

use app::TauriApp;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    let document = web_sys::window().unwrap().document().unwrap();
    let element = document
        .get_element_by_id("app")
        .expect("Failed to find app element");
    yew::Renderer::<TauriApp>::with_root(element).render();
}
