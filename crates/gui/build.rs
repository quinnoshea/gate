fn main() {
    // GUI serves frontend-tauri via Tauri's built-in asset serving
    // No need to manually copy frontend assets
    tauri_build::build()
}
