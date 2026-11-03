//! Install plans: the requested version plus its required dependencies, resolved level by level for
//! one instance. Installed projects are kept and not followed; incompatible ones are reported as conflicts.

use std::collections::{HashMap, HashSet};

use cubby_common::{Error, Result};

use crate::api;
use crate::browse::matches_loaders;
use crate::client::Modrinth;
use crate::model::*;

/// What resolution needs from Modrinth. Implemented by [`Modrinth`]; tests use fixtures.
pub(crate) trait Api {
    async fn get_versions(&self, ids: &[String]) -> Result<Vec<api::Version>>;
    async fn get_projects(&self, ids: &[String]) -> Result<Vec<api::Project>>;
    async fn get_project_versions(&self, project_id: &str, loaders: &[String], game_versions: &[String]) -> Result<Vec<api::Version>>;
}

impl Api for Modrinth {
    async fn get_versions(&self, ids: &[String]) -> Result<Vec<api::Version>> {
        self.versions_raw(ids).await
    }

    async fn get_projects(&self, ids: &[String]) -> Result<Vec<api::Project>> {
        self.projects_raw(ids).await
    }

    async fn get_project_versions(&self, project_id: &str, loaders: &[String], game_versions: &[String]) -> Result<Vec<api::Version>> {
        self.project_versions_raw(project_id, loaders, game_versions).await
    }
}

/// A project already in the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Installed {
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub file_name: String,
    pub folder: Folder,
    pub size: u64,
    pub enabled: bool,
}

/// Whether `version` can be installed for `target` as a project of `kind`.
pub(crate) fn compatible(version: &api::Version, target: &Target, kind: ProjectKind) -> bool {
    if !version.game_versions.contains(&target.game_version) {
        return false;
    }
    match kind {
        ProjectKind::Mod | ProjectKind::Modpack => {
            let allowed = target.mod_loaders();
            version.loaders.iter().any(|l| allowed.contains(l))
        }
        ProjectKind::ResourcePack | ProjectKind::Shader => matches_loaders(version, &[]),
    }
}

/// Newest compatible release, else newest compatible version of any channel.
pub(crate) fn pick_version<'a>(versions: &'a [api::Version], target: &Target, kind: ProjectKind) -> Option<&'a api::Version> {
    let mut ok: Vec<&api::Version> = versions.iter().filter(|v| compatible(v, target, kind)).collect();
    ok.sort_by_key(|v| std::cmp::Reverse(v.published_millis()));
    ok.iter().find(|v| v.version_type == "release" || v.version_type.is_empty()).or(ok.first()).copied()
}

/// A plain file name (no separators, not `.`/`..`).
pub(crate) fn plain_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0'])
}

fn kind_of(projects: &HashMap<String, api::Project>, project_id: &str) -> ProjectKind {
    projects.get(project_id).map(|p| ProjectKind::from_api(&p.project_type)).unwrap_or(ProjectKind::Mod)
}

fn title_of(projects: &HashMap<String, api::Project>, project_id: &str) -> Option<String> {
    projects.get(project_id).map(|p| p.title.clone()).filter(|t| !t.is_empty())
}

async fn fetch_projects<A: Api>(api: &A, cache: &mut HashMap<String, api::Project>, ids: impl IntoIterator<Item = String>) -> Result<()> {
    let mut wanted: Vec<String> = ids.into_iter().filter(|id| !cache.contains_key(id)).collect();
    wanted.sort();
    wanted.dedup();
    if wanted.is_empty() {
        return Ok(());
    }
    for p in api.get_projects(&wanted).await? {
        cache.insert(p.id.clone(), p);
    }
    Ok(())
}

fn item_for(version: &api::Version, reason: Reason, projects: &HashMap<String, api::Project>, installed: &HashMap<String, Installed>) -> Result<PlanItem> {
    let file = version.primary_file().ok_or_else(|| Error::Other(format!("Modrinth version {} has no file to download.", version.id)))?;
    if !plain_file_name(&file.filename) {
        return Err(Error::Other(format!("Modrinth version {} has an invalid file name: {:?}.", version.id, file.filename)));
    }
    let sha1 = file.sha1().filter(|h| h.len() == 40).ok_or_else(|| Error::Other(format!("Modrinth has no SHA-1 for {}, so it can't be verified.", file.filename)))?;
    let kind = kind_of(projects, &version.project_id);
    let folder = kind.folder().unwrap_or(Folder::Mods);
    let present = installed.get(&version.project_id);
    let same = present.is_some_and(|i| i.version_id == version.id);
    Ok(PlanItem {
        project_id: version.project_id.clone(),
        version_id: version.id.clone(),
        title: title_of(projects, &version.project_id).unwrap_or_else(|| version.name.clone()),
        version_number: version.version_number.clone(),
        file_name: if same { present.map(|i| i.file_name.clone()).unwrap_or_default() } else { file.filename.clone() },
        size: file.size,
        url: if same { String::new() } else { file.url.clone() },
        sha1: if same { String::new() } else { sha1.to_lowercase() },
        reason,
        installed: present.is_some(),
        folder,
        download: !same,
        replaces: present.filter(|_| !same).map(|i| (i.folder, i.file_name.clone())),
    })
}

fn installed_item(inst: &Installed, projects: &HashMap<String, api::Project>) -> PlanItem {
    PlanItem {
        project_id: inst.project_id.clone(),
        version_id: inst.version_id.clone(),
        title: title_of(projects, &inst.project_id).unwrap_or_else(|| inst.file_name.clone()),
        version_number: inst.version_number.clone(),
        file_name: inst.file_name.clone(),
        size: inst.size,
        url: String::new(),
        sha1: String::new(),
        reason: Reason::Dependency,
        installed: true,
        folder: inst.folder,
        download: false,
        replaces: None,
    }
}

/// Plans installing `version_id` into an instance with these projects installed.
pub(crate) async fn resolve<A: Api>(api: &A, version_id: &str, target: &Target, installed: &HashMap<String, Installed>) -> Result<InstallPlan> {
    let root = api
        .get_versions(&[version_id.to_string()])
        .await?
        .into_iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| Error::NotFound(format!("Modrinth version {version_id}")))?;
    let mut projects = HashMap::new();
    fetch_projects(api, &mut projects, [root.project_id.clone()]).await?;
    if kind_of(&projects, &root.project_id) == ProjectKind::Modpack {
        return Err(Error::Other("A modpack is installed as a new instance, not into an existing one.".into()));
    }

    let mut items = vec![item_for(&root, Reason::Requested, &projects, installed)?];
    let mut missing: Vec<MissingDependency> = Vec::new();
    let mut incompatible: Vec<(Option<String>, Option<String>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::from([root.project_id.clone()]);
    let mut level = vec![root];

    while !level.is_empty() {
        let mut explicit: Vec<(String, Option<String>)> = Vec::new();
        let mut by_project: Vec<String> = Vec::new();
        for v in &level {
            for d in &v.dependencies {
                match d.dependency_type.as_str() {
                    "required" => match (&d.version_id, &d.project_id) {
                        (Some(vid), pid) => explicit.push((vid.clone(), pid.clone())),
                        (None, Some(pid)) => by_project.push(pid.clone()),
                        (None, None) => {}
                    },
                    "incompatible" => incompatible.push((d.project_id.clone(), d.version_id.clone())),
                    _ => {}
                }
            }
        }
        let mut next = Vec::new();

        // Pinned versions. A pinned version that no longer exists falls back to the project's best version.
        explicit.retain(|(_, pid)| pid.as_ref().is_none_or(|p| !seen.contains(p)));
        if !explicit.is_empty() {
            let ids: Vec<String> = explicit.iter().map(|(v, _)| v.clone()).collect();
            let found = api.get_versions(&ids).await?;
            for (vid, pid) in &explicit {
                if let Some(pid) = pid.as_ref().filter(|_| !found.iter().any(|v| v.id == *vid)) {
                    by_project.push(pid.clone());
                }
            }
            fetch_projects(api, &mut projects, found.iter().map(|v| v.project_id.clone())).await?;
            for v in found {
                if !seen.insert(v.project_id.clone()) {
                    continue;
                }
                if let Some(inst) = installed.get(&v.project_id) {
                    items.push(installed_item(inst, &projects));
                    continue;
                }
                items.push(item_for(&v, Reason::Dependency, &projects, installed)?);
                next.push(v);
            }
        }

        // Projects: newest compatible version.
        let mut pids: Vec<String> = Vec::new();
        for pid in by_project {
            if !seen.contains(&pid) && !pids.contains(&pid) {
                pids.push(pid);
            }
        }
        fetch_projects(api, &mut projects, pids.iter().cloned()).await?;
        let game_versions = [target.game_version.clone()];
        // Owned ids: a closure taking `&String` makes the future not provably `Send` (rust-lang/rust#102211).
        let wanted: Vec<String> = pids.iter().filter(|pid| !installed.contains_key(*pid)).cloned().collect();
        let lookups = wanted.into_iter().map(|pid: String| {
            let kind = kind_of(&projects, &pid);
            let loaders = if matches!(kind, ProjectKind::Mod | ProjectKind::Modpack) { target.mod_loaders() } else { Vec::new() };
            let game_versions = &game_versions;
            async move { api.get_project_versions(&pid, &loaders, game_versions).await.map(|vs| (pid, vs)) }
        });
        let mut candidates: HashMap<String, Vec<api::Version>> = futures::future::try_join_all(lookups).await?.into_iter().collect();
        for pid in pids {
            seen.insert(pid.clone());
            if let Some(inst) = installed.get(&pid) {
                items.push(installed_item(inst, &projects));
                continue;
            }
            let versions = candidates.remove(&pid).unwrap_or_default();
            match pick_version(&versions, target, kind_of(&projects, &pid)) {
                Some(v) => {
                    items.push(item_for(v, Reason::Dependency, &projects, installed)?);
                    next.push(v.clone());
                }
                None => missing.push(MissingDependency { title: title_of(&projects, &pid), project_id: pid }),
            }
        }
        level = next;
    }

    // Conflicts with installed projects.
    let version_only: Vec<String> = incompatible.iter().filter(|(p, v)| p.is_none() && v.is_some()).filter_map(|(_, v)| v.clone()).collect();
    let version_projects: HashMap<String, String> = if version_only.is_empty() {
        HashMap::new()
    } else {
        api.get_versions(&version_only).await?.into_iter().map(|v| (v.id, v.project_id)).collect()
    };
    let mut conflicts: Vec<Conflict> = Vec::new();
    for (pid, vid) in incompatible {
        let Some(pid) = pid.or_else(|| vid.as_ref().and_then(|v| version_projects.get(v).cloned())) else { continue };
        let Some(inst) = installed.get(&pid) else { continue };
        if vid.as_ref().is_some_and(|v| *v != inst.version_id) || conflicts.iter().any(|c| c.project_id == pid) {
            continue;
        }
        conflicts.push(Conflict { project_id: pid, title: None, file_name: inst.file_name.clone() });
    }
    fetch_projects(api, &mut projects, conflicts.iter().map(|c| c.project_id.clone())).await?;
    for c in &mut conflicts {
        c.title = title_of(&projects, &c.project_id);
    }

    let download_bytes = items.iter().filter(|i| i.download).map(|i| i.size).sum();
    Ok(InstallPlan { items, missing, conflicts, download_bytes })
}

impl Modrinth {
    /// Plans installing a version into the instance whose game directory is `game_dir`. Nothing is changed.
    pub async fn plan_install(&self, version_id: &str, target: &Target, game_dir: &std::path::Path) -> Result<InstallPlan> {
        crate::client::check_id(version_id)?;
        let installed = self.installed(game_dir).await?;
        resolve(self, version_id, target, &installed).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Modrinth data from `tests/fixtures/resolve.json`, with a log of the requests made.
    struct Fixture {
        projects: Vec<api::Project>,
        versions: Vec<api::Version>,
        calls: Mutex<Vec<String>>,
    }

    impl Fixture {
        fn load() -> Self {
            #[derive(serde::Deserialize)]
            struct File {
                projects: Vec<api::Project>,
                versions: Vec<api::Version>,
            }
            let f: File = serde_json::from_str(include_str!("../tests/fixtures/resolve.json")).unwrap();
            Self { projects: f.projects, versions: f.versions, calls: Mutex::new(Vec::new()) }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Api for Fixture {
        async fn get_versions(&self, ids: &[String]) -> Result<Vec<api::Version>> {
            self.calls.lock().unwrap().push(format!("versions {}", ids.join(",")));
            Ok(self.versions.iter().filter(|v| ids.contains(&v.id)).cloned().collect())
        }

        async fn get_projects(&self, ids: &[String]) -> Result<Vec<api::Project>> {
            self.calls.lock().unwrap().push(format!("projects {}", ids.join(",")));
            Ok(self.projects.iter().filter(|p| ids.contains(&p.id)).cloned().collect())
        }

        async fn get_project_versions(&self, project_id: &str, loaders: &[String], game_versions: &[String]) -> Result<Vec<api::Version>> {
            self.calls.lock().unwrap().push(format!("project_versions {project_id} [{}] [{}]", loaders.join(","), game_versions.join(",")));
            Ok(self
                .versions
                .iter()
                .filter(|v| v.project_id == project_id)
                .filter(|v| loaders.is_empty() || v.loaders.iter().any(|l| loaders.contains(l)))
                .filter(|v| game_versions.is_empty() || v.game_versions.iter().any(|g| game_versions.contains(g)))
                .cloned()
                .collect())
        }
    }

    fn installed(entries: &[(&str, &str, &str)]) -> HashMap<String, Installed> {
        entries
            .iter()
            .map(|(project, version, file)| {
                (
                    project.to_string(),
                    Installed {
                        project_id: project.to_string(),
                        version_id: version.to_string(),
                        version_number: format!("{version}-number"),
                        file_name: file.to_string(),
                        folder: Folder::Mods,
                        size: 7,
                        enabled: true,
                    },
                )
            })
            .collect()
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(f)
    }

    fn summary(plan: &InstallPlan) -> Vec<(String, String, Reason, bool, bool)> {
        plan.items.iter().map(|i| (i.project_id.clone(), i.version_id.clone(), i.reason, i.installed, i.download)).collect()
    }

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn resolves_dependency_tree() {
        let api = Fixture::load();
        let target = Target::new("1.21.1", Some("fabric"));
        let plan = block_on(resolve(&api, "app-2", &target, &HashMap::new())).unwrap();
        // app-2 requires lib (newest release for 1.21.1 fabric: lib-3, not the newer beta lib-4),
        // pinned core-1 and the missing "ghost"; lib-3 requires base; optional "extra" is ignored.
        assert_eq!(
            summary(&plan),
            [
                (s("app"), s("app-2"), Reason::Requested, false, true),
                (s("core"), s("core-1"), Reason::Dependency, false, true),
                (s("lib"), s("lib-3"), Reason::Dependency, false, true),
                (s("base"), s("base-1"), Reason::Dependency, false, true),
            ]
        );
        assert_eq!(plan.missing, [MissingDependency { project_id: s("ghost"), title: Some(s("Ghost Lib")) }]);
        assert!(plan.conflicts.is_empty());
        assert_eq!(plan.download_bytes, 1000 + 300 + 200 + 100);
        let app = &plan.items[0];
        assert_eq!((app.title.as_str(), app.file_name.as_str(), app.folder), ("App", "app-2.0.jar", Folder::Mods));
        assert_eq!(app.sha1, "a".repeat(40));
        assert_eq!(app.url, "https://cdn.modrinth.com/app-2.0.jar");
        assert_eq!(plan.items[1].reason.as_str(), "dependency");
        // Lookups are batched per level, and the dependency's loaders are the instance's.
        let calls = api.calls();
        assert!(calls.contains(&s("project_versions lib [fabric] [1.21.1]")), "{calls:?}");
        assert!(calls.contains(&s("project_versions ghost [fabric] [1.21.1]")), "{calls:?}");
    }

    #[test]
    fn quilt_accepts_fabric_and_skips_installed() {
        let api = Fixture::load();
        let target = Target::new("1.21.1", Some("quilt"));
        // lib installed in another version: kept, not followed (base is not pulled in).
        let have = installed(&[("lib", "lib-1", "lib-1.0.jar"), ("app", "app-1", "app-1.0.jar.disabled")]);
        let plan = block_on(resolve(&api, "app-2", &target, &have)).unwrap();
        assert_eq!(
            summary(&plan),
            [
                (s("app"), s("app-2"), Reason::Requested, true, true),
                (s("core"), s("core-1"), Reason::Dependency, false, true),
                (s("lib"), s("lib-1"), Reason::Dependency, true, false),
            ]
        );
        // The requested project replaces its old file; a kept dependency shows the installed file.
        assert_eq!(plan.items[0].replaces, Some((Folder::Mods, s("app-1.0.jar.disabled"))));
        assert_eq!((plan.items[2].file_name.as_str(), plan.items[2].version_number.as_str()), ("lib-1.0.jar", "lib-1-number"));
        assert_eq!(plan.download_bytes, 1000 + 300);
        assert!(api.calls().iter().any(|c| c == "project_versions ghost [quilt,fabric] [1.21.1]"));

        // The very same version installed: nothing to download for it.
        let have = installed(&[("app", "app-2", "app-2.0.jar"), ("core", "core-1", "core.jar"), ("lib", "lib-3", "lib.jar")]);
        let plan = block_on(resolve(&api, "app-2", &target, &have)).unwrap();
        assert!(plan.items.iter().all(|i| !i.download && i.installed));
        assert_eq!(plan.items[0].replaces, None);
        assert_eq!(plan.download_bytes, 0);
    }

    #[test]
    fn conflicts_missing_and_kinds() {
        let api = Fixture::load();
        let target = Target::new("1.21.1", Some("fabric"));
        // shader-1 declares "app" incompatible (any version) and app-1 only via version id.
        let have = installed(&[("app", "app-1", "app-1.0.jar"), ("other", "other-1", "other.jar")]);
        let plan = block_on(resolve(&api, "shader-1", &target, &have)).unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!((plan.items[0].folder, plan.items[0].reason), (Folder::ShaderPacks, Reason::Requested));
        assert_eq!(plan.conflicts, [Conflict { project_id: s("app"), title: Some(s("App")), file_name: s("app-1.0.jar") }]);

        // Resource pack dependency resolved without the mod loader filter.
        let plan = block_on(resolve(&api, "rp-addon-1", &target, &HashMap::new())).unwrap();
        assert_eq!(summary(&plan)[1], (s("rp"), s("rp-2"), Reason::Dependency, false, true));
        assert_eq!(plan.items[1].folder, Folder::ResourcePacks);
        assert!(api.calls().iter().any(|c| c == "project_versions rp [] [1.21.1]"));

        // Nothing for vanilla: every mod dependency is missing.
        let plan = block_on(resolve(&api, "app-2", &Target::new("1.21.1", Some("vanilla")), &HashMap::new())).unwrap();
        assert_eq!(plan.missing.iter().map(|m| m.project_id.as_str()).collect::<Vec<_>>(), ["lib", "ghost"]);

        // Wrong game version: the newest compatible lib is lib-2.
        let plan = block_on(resolve(&api, "app-2", &Target::new("1.20.1", Some("fabric")), &HashMap::new())).unwrap();
        assert!(plan.items.iter().any(|i| i.version_id == "lib-2"));

        assert!(matches!(block_on(resolve(&api, "nope", &target, &HashMap::new())), Err(Error::NotFound(_))));
        let err = block_on(resolve(&api, "pack-1", &target, &HashMap::new())).unwrap_err();
        assert!(err.to_string().contains("modpack"), "{err}");
        assert!(block_on(resolve(&api, "evil-1", &target, &HashMap::new())).is_err(), "file names with separators are refused");
    }

    #[test]
    fn dependency_cycles_terminate() {
        let api = Fixture::load();
        let plan = block_on(resolve(&api, "cycle-a-1", &Target::new("1.21.1", Some("fabric")), &HashMap::new())).unwrap();
        assert_eq!(plan.items.iter().map(|i| i.project_id.as_str()).collect::<Vec<_>>(), ["cycle-a", "cycle-b"]);
    }

    #[test]
    fn picking_versions() {
        let v = |id: &str, kind: &str, date: &str, loaders: &[&str]| api::Version {
            id: id.into(),
            version_type: kind.into(),
            date_published: date.into(),
            game_versions: vec!["1.21.1".into()],
            loaders: loaders.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let target = Target::new("1.21.1", Some("fabric"));
        let versions = [
            v("old", "release", "2024-01-01T00:00:00Z", &["fabric"]),
            v("new-beta", "beta", "2024-06-01T00:00:00.123456Z", &["fabric"]),
            v("forge", "release", "2024-07-01T00:00:00Z", &["forge"]),
            v("mid", "release", "2024-03-01T00:00:00Z", &["fabric"]),
        ];
        assert_eq!(pick_version(&versions, &target, ProjectKind::Mod).unwrap().id, "mid");
        assert_eq!(pick_version(&versions[1..2], &target, ProjectKind::Mod).unwrap().id, "new-beta");
        assert!(pick_version(&versions[2..3], &target, ProjectKind::Mod).is_none());
        assert_eq!(pick_version(&[v("rp", "release", "2024-01-01T00:00:00Z", &["minecraft"])], &target, ProjectKind::ResourcePack).unwrap().id, "rp");
        assert!(!plain_file_name("../a.jar") && !plain_file_name("a/b.jar") && plain_file_name("a.jar"));
    }
}
