//! Turning one version of a pack into a [`PackPlan`].

use std::path::{Path, PathBuf};

use cubby_common::packs::{safe_relative_path, BlockedFile, PackFile, PackPlan};
use cubby_common::{Error, Result};

use crate::api;
use crate::browse::{icon_of, loader_of, target_version};
use crate::client::{parse_id, FtbClient};

/// Page of a CurseForge project, where the player can pick the file. CurseForge file pages need the
/// project slug, which FTB doesn't give; this address redirects to the project from its numeric id.
pub fn curseforge_page_url(project: &str) -> String {
    format!("https://www.curseforge.com/projects/{project}")
}

impl FtbClient {
    /// Everything needed to install version `version_id` of pack `pack_id`.
    ///
    /// FTB packs are a flat list of files (mods, configs, scripts...), so the plan has no overrides and
    /// `cache_dir` is unused. Server-only files are left out. A file FTB can't serve (empty URL) that
    /// comes from CurseForge goes to `blocked`; one with no way to get it is an error.
    pub async fn plan(&self, pack_id: &str, version_id: &str, _cache_dir: &Path) -> Result<PackPlan> {
        let pack_n = parse_id(pack_id, "modpack")?;
        let version_n = parse_id(version_id, "version")?;
        let (pack, version) = futures::try_join!(self.pack_raw(pack_n), self.version_raw(pack_n, version_n))?;
        build_plan(&pack, &version)
    }
}

pub(crate) fn build_plan(pack: &api::Pack, version: &api::VersionDetail) -> Result<PackPlan> {
    let targets = &version.stub.targets;
    let minecraft = target_version(targets, "minecraft")
        .ok_or_else(|| Error::Other(format!("{} {} doesn't say which Minecraft version it needs.", pack.name, version.stub.name)))?
        .to_string();

    let mut files = Vec::new();
    let mut blocked = Vec::new();
    for file in version.files.iter().filter(|f| !f.serveronly) {
        let path = file_path(file)?;
        let sha1 = Some(file.sha1.to_lowercase()).filter(|s| !s.is_empty());
        let mut urls: Vec<String> = Vec::new();
        for url in std::iter::once(&file.url).chain(&file.mirrors) {
            if (url.starts_with("https://") || url.starts_with("http://")) && !urls.contains(url) {
                urls.push(url.clone());
            }
        }
        if !urls.is_empty() {
            files.push(PackFile { path, urls, sha1, size: file.size });
        } else if let Some(cf) = &file.curseforge {
            blocked.push(BlockedFile { file_name: file.name.clone(), page_url: curseforge_page_url(&cf.project.clone().into_string()), path, sha1, size: file.size });
        } else {
            return Err(Error::Other(format!("{} has no download address in {} {}.", file.name, pack.name, version.stub.name)));
        }
    }

    Ok(PackPlan {
        provider: "ftb".to_string(),
        pack_id: pack.id.to_string(),
        version_id: version.stub.id.to_string(),
        title: pack.name.clone(),
        version_name: version.stub.name.clone(),
        minecraft,
        loader: loader_of(targets),
        icon_url: icon_of(&pack.art),
        files,
        overrides: Vec::new(),
        blocked,
        recommended_memory_mib: version.stub.specs.as_ref().and_then(|s| s.recommended.filter(|m| *m > 0).or(s.minimum.filter(|m| *m > 0))),
    })
}

/// `./mods` + `x.jar` gives `mods/x.jar`; anything that would leave the game directory is refused.
fn file_path(file: &api::File) -> Result<PathBuf> {
    let bad = || Error::Other(format!("The pack contains an unsafe file path ({:?} in {:?}); installation was refused.", file.name, file.path));
    if file.name.is_empty() || file.name.contains(['/', '\\']) || file.name == ".." || file.name == "." {
        return Err(bad());
    }
    safe_relative_path(&format!("{}/{}", file.path, file.name)).ok_or_else(bad)
}
