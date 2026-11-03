//! Installing a modpack plan from any provider. It lives outside the Tauri crate because a test
//! binary linking Tauri does not start on Windows.

pub mod checks;
pub mod install;
pub mod text;
