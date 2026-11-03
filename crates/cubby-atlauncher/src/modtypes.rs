//! ATLauncher mod types (<https://wiki.atlauncher.com/mod_types>) and the folder each one installs to,
//! as Prism Launcher's `PackInstallTask::getDirForModType` maps them.

/// The `type` of a mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    /// Game directory itself.
    Root,
    /// Minecraft Forge as a mod: becomes the pack's loader (a pack without a `loader` block only).
    Forge,
    /// Patches minecraft.jar: unsupported.
    Jar,
    Mods,
    Flan,
    /// `mods/<minecraft version>`.
    Dependency,
    Ic2Lib,
    DenLib,
    Coremods,
    /// MCPC+ server jar: never installed on a client.
    Mcpc,
    Plugins,
    /// Zip extracted into the folder of `extractTo`.
    Extract,
    /// One file taken out of a zip, into the folder of `decompType`.
    Decomp,
    TexturePack,
    ResourcePack,
    ShaderPack,
    /// Zip extracted into `texturepacks/extracted`.
    TexturePackExtract,
    /// Zip extracted into `resourcepacks/extracted`.
    ResourcePackExtract,
    /// Prism skips it with a warning; so does the plan.
    Millenaire,
    Unknown,
}

impl ModKind {
    /// Reads the `type` string (case-insensitive; "depandency" is a typo that occurs in real packs).
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "root" => Self::Root,
            "forge" => Self::Forge,
            "jar" => Self::Jar,
            "mods" => Self::Mods,
            "flan" => Self::Flan,
            "dependency" | "depandency" => Self::Dependency,
            "ic2lib" => Self::Ic2Lib,
            "denlib" => Self::DenLib,
            "coremods" => Self::Coremods,
            "mcpc" => Self::Mcpc,
            "plugins" => Self::Plugins,
            "extract" => Self::Extract,
            "decomp" => Self::Decomp,
            "texturepack" => Self::TexturePack,
            "resourcepack" => Self::ResourcePack,
            "shaderpack" => Self::ShaderPack,
            "texturepackextract" => Self::TexturePackExtract,
            "resourcepackextract" => Self::ResourcePackExtract,
            "millenaire" => Self::Millenaire,
            _ => Self::Unknown,
        }
    }

    /// Folder (relative to the game directory, `/`-separated, empty for the root) a plain file of this
    /// type goes to; `None` for types that aren't a plain file (extract, decomp, forge, jar...).
    pub fn folder(self, minecraft: &str) -> Option<String> {
        Some(match self {
            Self::Root => String::new(),
            Self::Mods => "mods".into(),
            Self::Flan => "Flan".into(),
            Self::Dependency => format!("mods/{minecraft}"),
            Self::Ic2Lib => "mods/ic2".into(),
            Self::DenLib => "mods/denlib".into(),
            Self::Coremods => "coremods".into(),
            Self::Plugins => "plugins".into(),
            Self::TexturePack => "texturepacks".into(),
            Self::ResourcePack => "resourcepacks".into(),
            Self::ShaderPack => "shaderpacks".into(),
            _ => return None,
        })
    }

    /// Types that arrive as a zip to unpack.
    pub fn is_archive(self) -> bool {
        matches!(self, Self::Extract | Self::Decomp | Self::TexturePackExtract | Self::ResourcePackExtract)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_folders() {
        assert_eq!(ModKind::parse("Mods"), ModKind::Mods);
        assert_eq!(ModKind::parse("depandency"), ModKind::Dependency);
        assert_eq!(ModKind::parse("whatever"), ModKind::Unknown);
        assert_eq!(ModKind::Dependency.folder("1.7.10").as_deref(), Some("mods/1.7.10"));
        assert_eq!(ModKind::Ic2Lib.folder("1.7.10").as_deref(), Some("mods/ic2"));
        assert_eq!(ModKind::ResourcePack.folder("1.20.1").as_deref(), Some("resourcepacks"));
        assert_eq!(ModKind::Root.folder("1.20.1").as_deref(), Some(""));
        assert_eq!(ModKind::Jar.folder("1.20.1"), None);
        assert!(ModKind::Decomp.is_archive() && !ModKind::Mods.is_archive());
    }
}
