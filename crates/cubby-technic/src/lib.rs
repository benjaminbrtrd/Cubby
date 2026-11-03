//! Technic Platform modpacks: browse packs and plan installs of both single-zip and Solder packs.

mod client;
mod detect;
mod model;
mod plan;

pub use client::{TechnicClient, API_BASE, API_BUILD};
pub use detect::{detect, Detected};
pub use model::{SolderBuild, SolderMod, TechnicPack};
pub use plan::TechnicPlan;
