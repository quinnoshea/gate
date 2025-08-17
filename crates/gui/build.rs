use std::fs;
use std::path::Path;

fn main() {
    // Copy frontend-daemon dist files to resources directory
    let frontend_daemon_dist = Path::new("../frontend-daemon/dist");
    let resources_dir = Path::new("resources/frontend-daemon");

    // Remove old resources directory if it exists
    if resources_dir.exists() {
        fs::remove_dir_all(resources_dir).expect("Failed to remove old resources");
    }

    // Always create resources directory, even if empty
    fs::create_dir_all(resources_dir).expect("Failed to create resources directory");

    // Only copy if frontend-daemon dist exists
    if frontend_daemon_dist.exists() {
        println!("cargo:rerun-if-changed=../frontend-daemon/dist");

        // Copy all files from frontend-daemon/dist to resources/frontend-daemon
        copy_dir_all(frontend_daemon_dist, resources_dir)
            .expect("Failed to copy frontend-daemon files");

        println!("cargo:warning=Copied frontend-daemon files to resources directory");
    } else {
        println!("cargo:warning=frontend-daemon dist directory not found, skipping resource copy");
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
