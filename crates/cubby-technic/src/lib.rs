//! Technic Platform modpacks: browse packs and plan installs of both single-zip and Solder packs.

mod client;
mod contents;
mod detect;
mod model;
mod plan;

pub use contents::build_contents;
pub use client::{TechnicClient, API_BASE, API_BUILD};
pub use detect::{detect, Detected};
pub use model::{SolderBuild, SolderMod, TechnicPack};
pub use plan::TechnicPlan;
