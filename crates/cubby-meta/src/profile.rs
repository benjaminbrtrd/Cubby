//! Merging a loader profile onto its parent version (`inheritsFrom`).

use std::collections::HashSet;

use cubby_common::version::{Arguments, VersionJson};

/// Key identifying a library regardless of its version: `group:artifact[:classifier]`.
pub fn library_key(name: &str) -> String {
    let coords = name.split_once('@').map_or(name, |(c, _)| c);
    let mut parts = coords.split(':');
    let group = parts.next().unwrap_or_default();
    let artifact = parts.next().unwrap_or_default();
    let _version = parts.next();
    match parts.next() {
        Some(classifier) => format!("{group}:{artifact}:{classifier}"),
        None => format!("{group}:{artifact}"),
    }
}

/// Applies `child` (a profile with `inheritsFrom`) on top of `parent`, like the official launcher.
/// Libraries are the child's first, then the parent's, de-duplicated by [`library_key`]; arguments are the
/// parent's then the child's; everything else is the child's when present. `inherits_from` is kept.
pub fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    let mut seen = HashSet::new();
    let libraries = child
        .libraries
        .into_iter()
        .chain(parent.libraries)
        .filter(|lib| seen.insert(library_key(&lib.name)))
        .collect();

    let arguments = match (parent.arguments, child.arguments) {
        (None, None) => None,
        (p, c) => {
            let (p, c) = (p.unwrap_or_default(), c.unwrap_or_default());
            Some(Arguments { game: [p.game, c.game].concat(), jvm: [p.jvm, c.jvm].concat() })
        }
    };

    VersionJson {
        id: if child.id.is_empty() { parent.id } else { child.id },
        inherits_from: child.inherits_from.or(parent.inherits_from),
        kind: child.kind.or(parent.kind),
        main_class: child.main_class.or(parent.main_class),
        arguments,
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        libraries,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        release_time: child.release_time.or(parent.release_time),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubby_common::version::{Argument, Library};

    fn lib(name: &str) -> Library {
        Library { name: name.into(), ..Default::default() }
    }

    fn plain(s: &str) -> Argument {
        Argument::Plain(s.into())
    }

    fn texts(args: &[Argument]) -> Vec<&str> {
        args.iter().map(|a| if let Argument::Plain(s) = a { s.as_str() } else { "?" }).collect()
    }

    #[test]
    fn keys() {
        assert_eq!(library_key("org.ow2.asm:asm:9.6"), "org.ow2.asm:asm");
        assert_eq!(library_key("org.lwjgl:lwjgl:3.3.3:natives-windows"), "org.lwjgl:lwjgl:natives-windows");
        assert_eq!(library_key("a:b:1@zip"), "a:b");
    }

    #[test]
    fn merge_semantics() {
        let parent = VersionJson {
            id: "1.21.1".into(),
            kind: Some("release".into()),
            main_class: Some("net.minecraft.client.main.Main".into()),
            arguments: Some(Arguments { game: vec![plain("--username")], jvm: vec![plain("-Xss1M")] }),
            minecraft_arguments: Some("parent-legacy".into()),
            libraries: vec![lib("org.ow2.asm:asm:9.6"), lib("com.x:y:1"), lib("org.lwjgl:lwjgl:3:natives-windows"), lib("org.lwjgl:lwjgl:3")],
            assets: Some("17".into()),
            release_time: Some("2024".into()),
            ..Default::default()
        };
        let child = VersionJson {
            id: "fabric-loader-0.20.1-1.21.1".into(),
            inherits_from: Some("1.21.1".into()),
            main_class: Some("KnotClient".into()),
            arguments: Some(Arguments { game: vec![plain("--fabric")], jvm: vec![] }),
            minecraft_arguments: Some("child-legacy".into()),
            libraries: vec![lib("org.ow2.asm:asm:9.10.1"), lib("net.fabricmc:loader:0.20.1")],
            ..Default::default()
        };
        let m = merge(parent, child);
        assert_eq!(m.id, "fabric-loader-0.20.1-1.21.1");
        assert_eq!(m.inherits_from.as_deref(), Some("1.21.1"));
        assert_eq!(m.main_class.as_deref(), Some("KnotClient"));
        assert_eq!(m.kind.as_deref(), Some("release"));
        assert_eq!(m.assets.as_deref(), Some("17"));
        assert_eq!(m.minecraft_arguments.as_deref(), Some("child-legacy"));
        let args = m.arguments.unwrap();
        assert_eq!(texts(&args.game), ["--username", "--fabric"]);
        assert_eq!(texts(&args.jvm), ["-Xss1M"]);
        let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            ["org.ow2.asm:asm:9.10.1", "net.fabricmc:loader:0.20.1", "com.x:y:1", "org.lwjgl:lwjgl:3:natives-windows", "org.lwjgl:lwjgl:3"]
        );
    }

    #[test]
    fn child_without_arguments_keeps_parent() {
        let parent = VersionJson { id: "p".into(), arguments: Some(Arguments { game: vec![plain("a")], jvm: vec![] }), ..Default::default() };
        let child = VersionJson { id: "c".into(), ..Default::default() };
        let m = merge(parent, child);
        assert_eq!(texts(&m.arguments.unwrap().game), ["a"]);
        let m = merge(VersionJson::default(), VersionJson { id: "c".into(), ..Default::default() });
        assert!(m.arguments.is_none());
    }
}
