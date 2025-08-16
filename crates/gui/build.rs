use std::fs;
use std::path::Path;

fn main() {
    // Copy frontend-tauri dist files to resources directory
    let frontend_tauri_dist = Path::new("../frontend-tauri/dist");
    let resources_dir = Path::new("resources/frontend-daemon");

    // Remove old resources directory if it exists
    if resources_dir.exists() {
        fs::remove_dir_all(resources_dir).expect("Failed to remove old resources");
    }

    // Always create resources directory, even if empty
    fs::create_dir_all(resources_dir).expect("Failed to create resources directory");

    // Only copy if frontend-tauri dist exists
    if frontend_tauri_dist.exists() {
        println!("cargo:rerun-if-changed=../frontend-tauri/dist");

        // Copy all files from frontend-tauri/dist to resources/frontend-daemon
        copy_dir_all(frontend_tauri_dist, resources_dir)
            .expect("Failed to copy frontend-tauri files");

        println!("cargo:warning=Copied frontend-tauri files to resources directory");
    } else {
        println!("cargo:warning=frontend-tauri dist directory not found, skipping resource copy");
    }

    tauri_build::build()
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_all(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}
