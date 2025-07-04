use gate_frontend_relay::RelayApp;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<RelayApp>::new().render();
}
