//! The optional-mods picker: which mods the player may toggle, and the selection rules Prism Launcher's
//! `AtlOptionalModDialog` applies (mods of one group exclude each other, enabling a mod enables what it
//! depends on, disabling a mod disables what depends on it).

use std::collections::{HashMap, HashSet};

use crate::model::{AtlManifest, AtlMod};

/// An optional mod, as shown in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalMod {
    /// Identifies the mod: pass it back in `selected_optional`.
    pub name: String,
    pub description: String,
    /// Selected when the picker opens.
    pub selected: bool,
    pub recommended: bool,
    /// Mods of one group exclude each other; empty for none.
    pub group: String,
    /// Names of the mods this one needs.
    pub depends: Vec<String>,
    /// Text to show before the player enables the mod ("This may damage worlds").
    pub warning: Option<String>,
    /// Hex RGB without `#` to tint the row with.
    pub colour: Option<String>,
    /// Hidden or library entry: Prism only turns it on and off through the mods that need it, so a picker
    /// may choose not to list it.
    pub hidden: bool,
}

/// The optional mods that get installed on a client, in manifest order.
pub fn optional_mods(manifest: &AtlManifest) -> Vec<OptionalMod> {
    manifest
        .mods
        .iter()
        .filter(|m| m.optional && m.client)
        .map(|m| OptionalMod {
            name: m.name.clone(),
            description: m.description.clone(),
            selected: m.selected,
            recommended: m.recommended,
            group: m.group.clone(),
            depends: m.depends.clone(),
            warning: manifest.warnings.get(&m.warning).cloned(),
            colour: manifest.colours.get(&m.colour).cloned(),
            hidden: m.hidden || m.library,
        })
        .collect()
}

/// A selection of optional mods that keeps itself consistent like Prism's dialog does.
#[derive(Debug, Clone)]
pub struct OptionalSelection {
    mods: Vec<OptionalMod>,
    index: HashMap<String, usize>,
    selected: HashSet<String>,
    /// For each mod, the enabled mods that need it.
    dependents: HashMap<String, Vec<String>>,
}

impl OptionalSelection {
    /// Starts from the mods the pack selects by default.
    pub fn new(manifest: &AtlManifest) -> Self {
        let mods = optional_mods(manifest);
        let index = mods.iter().enumerate().map(|(i, m)| (m.name.clone(), i)).collect();
        let mut selection = Self { mods, index, selected: HashSet::new(), dependents: HashMap::new() };
        for i in 0..selection.mods.len() {
            if selection.mods[i].selected {
                selection.set_at(i, true);
            }
        }
        selection
    }

    /// Names of the selected mods, in manifest order: what `plan` takes.
    pub fn names(&self) -> Vec<String> {
        self.mods.iter().filter(|m| self.selected.contains(&m.name)).map(|m| m.name.clone()).collect()
    }

    pub fn is_selected(&self, name: &str) -> bool {
        self.selected.contains(name)
    }

    /// Turns a mod on or off, with its group and dependency side effects. Unknown names are ignored.
    pub fn set(&mut self, name: &str, enable: bool) {
        if let Some(&i) = self.index.get(name) {
            self.set_at(i, enable);
        }
    }

    /// "Select recommended": exactly the recommended mods (no side effects, like Prism).
    pub fn select_recommended(&mut self) {
        self.selected = self.mods.iter().filter(|m| m.recommended).map(|m| m.name.clone()).collect();
        self.dependents.clear();
    }

    /// "Clear all".
    pub fn clear(&mut self) {
        self.selected.clear();
        self.dependents.clear();
    }

    fn set_at(&mut self, i: usize, enable: bool) {
        let this = self.mods[i].clone();
        if self.selected.contains(&this.name) == enable {
            return;
        }
        if enable {
            self.selected.insert(this.name.clone());
        } else {
            self.selected.remove(&this.name);
        }
        if enable && !this.group.is_empty() {
            for j in 0..self.mods.len() {
                if j != i && self.mods[j].group == this.group {
                    self.set_at(j, false);
                }
            }
        }
        for dependency in &this.depends {
            let Some(&d) = self.index.get(dependency) else { continue };
            if enable {
                self.set_at(d, true);
            }
            let dependents = self.dependents.entry(dependency.clone()).or_default();
            if enable {
                dependents.push(this.name.clone());
            } else {
                dependents.retain(|n| n != &this.name);
                if self.mods[d].hidden && dependents.is_empty() {
                    self.set_at(d, false);
                }
            }
        }
        if !enable {
            let dependents = self.dependents.get(&this.name).cloned().unwrap_or_default();
            for dependent in dependents {
                if let Some(&d) = self.index.get(&dependent) {
                    self.set_at(d, false);
                }
            }
        }
    }
}

/// Whether a mod is installed for this selection (Prism: client mods, optional ones only if selected).
pub(crate) fn is_installed(m: &AtlMod, selected: &HashSet<&str>) -> bool {
    m.client && (!m.optional || selected.contains(m.name.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> AtlManifest {
        serde_json::from_str(include_str!("../tests/fixtures/manifest_optional.json")).unwrap()
    }

    #[test]
    fn lists_optional_client_mods() {
        let mods = optional_mods(&manifest());
        let names: Vec<_> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Shaders Mod", "Minimap", "Minimap Lib", "Mini Map Alt", "Fast Craft"]);
        let fast = mods.iter().find(|m| m.name == "Fast Craft").unwrap();
        assert_eq!(fast.warning.as_deref(), Some("May damage worlds."));
        assert_eq!(fast.colour.as_deref(), Some("FF0000"));
        let lib = mods.iter().find(|m| m.name == "Minimap Lib").unwrap();
        assert!(lib.hidden);
    }

    #[test]
    fn selection_rules() {
        let m = manifest();
        let mut s = OptionalSelection::new(&m);
        assert_eq!(s.names(), ["Shaders Mod"]);
        // Enabling Minimap pulls its hidden library in.
        s.set("Minimap", true);
        assert_eq!(s.names(), ["Shaders Mod", "Minimap", "Minimap Lib"]);
        // The alternative is in the same group: it replaces Minimap, and the library goes with it.
        s.set("Mini Map Alt", true);
        assert_eq!(s.names(), ["Shaders Mod", "Mini Map Alt"]);
        s.set("Minimap", true);
        s.set("Minimap", false);
        assert_eq!(s.names(), ["Shaders Mod"]);
        s.select_recommended();
        assert_eq!(s.names(), ["Shaders Mod", "Minimap"]);
        s.clear();
        assert!(s.names().is_empty());
        s.set("Nope", true);
        assert!(s.names().is_empty());
    }
}
