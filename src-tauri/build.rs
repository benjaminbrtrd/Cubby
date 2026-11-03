fn main() {
    // Recompile native resources when branding changes, including the Windows
    // icon used by the executable, window and taskbar.
    println!("cargo:rerun-if-changed=icons");
  tauri_build::build()
}
