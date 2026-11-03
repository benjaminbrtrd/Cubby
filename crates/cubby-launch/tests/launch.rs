use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cubby_common::contracts::{InstalledVersion, JavaInstall, LaunchCredentials, NativeJar};
use cubby_common::events::{Event, EventSink, LogStream};
use cubby_common::version::{RuleEnv, VersionJson};
use cubby_launch::*;

const MODERN: &str = include_str!("fixtures/1.21.1.json");
const LEGACY: &str = include_str!("fixtures/1.8.9.json");
const TOKEN: &str = "SECRET-TOKEN-0123456789";

fn installed(json: &str) -> InstalledVersion {
    let version: VersionJson = serde_json::from_str(json).unwrap();
    let root = PathBuf::from(if cfg!(windows) { "C:/fx" } else { "/fx" });
    let env = RuleEnv::current();
    let libraries = version
        .libraries
        .iter()
        .filter(|l| l.applies(&env) && l.natives.is_none())
        .filter_map(|l| l.downloads.as_ref()?.artifact.as_ref()?.path.clone())
        .map(|p| root.join("libraries").join(p))
        .collect();
    let index = version.assets.clone().unwrap();
    InstalledVersion {
        client_jar: root.join("versions").join(&version.id).join(format!("{}.jar", version.id)),
        libraries,
        natives: vec![],
        assets_root: root.join("assets"),
        assets_index_id: index,
        game_assets_dir: root.join("assets"),
        logging_argument: version
            .logging
            .as_ref()
            .and_then(|l| l.client.as_ref())
            .map(|c| c.argument.replace("${path}", "C:/fx/log.xml")),
        version,
    }
}

fn creds() -> LaunchCredentials {
    LaunchCredentials {
        username: "Steve".into(),
        uuid: "11111111-2222-3333-4444-555555555555".into(),
        access_token: TOKEN.into(),
        user_type: "msa".into(),
        xuid: "42".into(),
    }
}

fn opts() -> LaunchOptions {
    LaunchOptions {
        instance_id: "inst".into(),
        game_dir: PathBuf::from("C:/games/inst/.minecraft"),
        java: JavaInstall {
            executable: PathBuf::from("java"),
            major: 21,
            version: "21.0.5".into(),
            arch: "x86_64".into(),
            vendor: None,
            managed: false,
        },
        min_memory_mb: 512,
        max_memory_mb: 4096,
        extra_jvm_args: vec!["-Dextra=1".into()],
        window: None,
        fullscreen: false,
        quick_play: None,
        demo: false,
    }
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).map(String::as_str)
}

fn no_placeholders(args: &[String]) {
    for a in args {
        assert!(!a.contains("${"), "unresolved placeholder in {a}");
    }
}

#[test]
fn modern_command() {
    let inst = installed(MODERN);
    let cmd = build_command(&inst, &creds(), &opts(), Path::new("C:/n")).unwrap();
    let a = &cmd.args;
    assert_eq!(a[0], "-Xms512M");
    assert_eq!(a[1], "-Xmx4096M");
    no_placeholders(a);

    let main = a.iter().position(|x| x == "net.minecraft.client.main.Main").unwrap();
    // JVM part: library path, brand, classpath, logging, extra all come before the main class.
    let jvm = &a[..main];
    assert!(jvm.contains(&"-Djava.library.path=C:/n".to_string()));
    assert!(jvm.contains(&"-Dminecraft.launcher.brand=Cubby".to_string()));
    assert!(jvm.contains(&format!("-Dminecraft.launcher.version={LAUNCHER_VERSION}")));
    assert!(jvm.iter().any(|x| x.starts_with("-Dlog4j.configurationFile=")));
    assert_eq!(jvm.last().unwrap(), "-Dextra=1");
    let log_pos = jvm.iter().position(|x| x.starts_with("-Dlog4j")).unwrap();
    assert!(log_pos < jvm.len() - 1);

    // Classpath: libraries in order, then client jar, right separator.
    let sep = if cfg!(windows) { ';' } else { ':' };
    let cp = value_after(a, "-cp").unwrap();
    let parts: Vec<_> = cp.split(sep).collect();
    assert_eq!(parts.len(), inst.libraries.len() + 1);
    assert!(parts.last().unwrap().ends_with("1.21.1.jar"));

    // Game part.
    let game = &a[main + 1..];
    assert_eq!(value_after(game, "--username"), Some("Steve"));
    assert_eq!(value_after(game, "--version"), Some("1.21.1"));
    assert_eq!(value_after(game, "--uuid"), Some("11111111-2222-3333-4444-555555555555"));
    assert_eq!(value_after(game, "--accessToken"), Some(TOKEN));
    assert_eq!(value_after(game, "--assetIndex"), Some(inst.assets_index_id.as_str()));
    assert_eq!(value_after(game, "--gameDir"), Some("C:/games/inst/.minecraft"));
    assert_eq!(value_after(game, "--userType"), Some("msa"));
    assert_eq!(value_after(game, "--versionType"), Some("release"));
    assert_eq!(value_after(game, "--xuid"), Some("42"));

    // Rule-filtered: nothing for features we did not enable.
    for flag in ["--demo", "--width", "--quickPlayPath", "--quickPlaySingleplayer", "--quickPlayMultiplayer", "--fullscreen"] {
        assert!(!game.contains(&flag.to_string()), "{flag} should be filtered out");
    }
    assert_eq!(cmd.cwd, PathBuf::from("C:/games/inst/.minecraft"));
    assert_eq!(cmd.secrets, vec![TOKEN.to_string()]);
    assert!(!format!("{cmd:?}").contains(TOKEN));
}

#[test]
fn modern_windows_only_args_follow_rules() {
    let inst = installed(MODERN);
    let cmd = build_command(&inst, &creds(), &opts(), Path::new("C:/n")).unwrap();
    let heap = cmd.args.iter().any(|a| a.starts_with("-XX:HeapDumpPath="));
    assert_eq!(heap, cfg!(windows));
    let xstart = cmd.args.iter().any(|a| a == "-XstartOnFirstThread");
    assert_eq!(xstart, cfg!(target_os = "macos"));
}

#[test]
fn modern_resolution_and_fullscreen() {
    let mut o = opts();
    o.window = Some((1280, 720));
    o.fullscreen = true;
    let cmd = build_command(&installed(MODERN), &creds(), &o, Path::new("C:/n")).unwrap();
    assert_eq!(value_after(&cmd.args, "--width"), Some("1280"));
    assert_eq!(value_after(&cmd.args, "--height"), Some("720"));
    assert_eq!(cmd.args.iter().filter(|a| *a == "--width").count(), 1);
    assert!(cmd.args.contains(&"--fullscreen".to_string()));
    no_placeholders(&cmd.args);
}

#[test]
fn modern_quick_play() {
    let mut o = opts();
    o.quick_play = Some(QuickPlay::Singleplayer("My World".into()));
    let cmd = build_command(&installed(MODERN), &creds(), &o, Path::new("C:/n")).unwrap();
    assert_eq!(value_after(&cmd.args, "--quickPlaySingleplayer"), Some("My World"));
    assert!(value_after(&cmd.args, "--quickPlayPath").is_some());
    assert!(!cmd.args.contains(&"--quickPlayMultiplayer".to_string()));
    no_placeholders(&cmd.args);

    o.quick_play = Some(QuickPlay::Multiplayer("play.example.com:25570".into()));
    let cmd = build_command(&installed(MODERN), &creds(), &o, Path::new("C:/n")).unwrap();
    assert_eq!(value_after(&cmd.args, "--quickPlayMultiplayer"), Some("play.example.com:25570"));
    assert!(!cmd.args.contains(&"--server".to_string()));
}

#[test]
fn legacy_command() {
    let inst = installed(LEGACY);
    let cmd = build_command(&inst, &creds(), &opts(), Path::new("C:/n")).unwrap();
    let a = &cmd.args;
    no_placeholders(a);
    assert_eq!(&a[..2], ["-Xms512M", "-Xmx4096M"]);
    assert_eq!(a.iter().any(|x| x.starts_with("-XX:HeapDumpPath=Mojang")), cfg!(windows));
    assert!(a.contains(&"-Djava.library.path=C:/n".to_string()));
    let main = a.iter().position(|x| x == "net.minecraft.client.main.Main").unwrap();
    assert!(a[..main].contains(&"-cp".to_string()));
    assert!(a[..main].contains(&"-Dextra=1".to_string()));
    let game = &a[main + 1..];
    assert_eq!(value_after(game, "--username"), Some("Steve"));
    assert_eq!(value_after(game, "--accessToken"), Some(TOKEN));
    assert_eq!(value_after(game, "--userProperties"), Some("{}"));
    assert_eq!(value_after(game, "--assetsDir"), Some(inst.assets_root.to_str().unwrap()));
    assert_eq!(value_after(game, "--assetIndex"), Some("1.8"));
}

#[test]
fn legacy_resolution_and_server() {
    let mut o = opts();
    o.window = Some((800, 600));
    o.quick_play = Some(QuickPlay::Multiplayer("mc.example.com".into()));
    let cmd = build_command(&installed(LEGACY), &creds(), &o, Path::new("C:/n")).unwrap();
    assert_eq!(value_after(&cmd.args, "--width"), Some("800"));
    assert_eq!(value_after(&cmd.args, "--height"), Some("600"));
    assert_eq!(value_after(&cmd.args, "--server"), Some("mc.example.com"));
    assert_eq!(value_after(&cmd.args, "--port"), Some("25565"));

    o.quick_play = Some(QuickPlay::Multiplayer("[::1]:1234".into()));
    let cmd = build_command(&installed(LEGACY), &creds(), &o, Path::new("C:/n")).unwrap();
    assert_eq!(value_after(&cmd.args, "--server"), Some("::1"));
    assert_eq!(value_after(&cmd.args, "--port"), Some("1234"));
}

#[test]
fn missing_main_class_is_an_error() {
    let mut inst = installed(MODERN);
    inst.version.main_class = None;
    assert!(matches!(
        build_command(&inst, &creds(), &opts(), Path::new("C:/n")),
        Err(cubby_common::Error::Launch(_))
    ));
}

#[test]
fn unknown_placeholders_are_left_alone() {
    let mut inst = installed(LEGACY);
    inst.version.minecraft_arguments = Some("--foo ${mystery} --user ${auth_player_name}".into());
    let cmd = build_command(&inst, &creds(), &opts(), Path::new("C:/n")).unwrap();
    assert_eq!(value_after(&cmd.args, "--foo"), Some("${mystery}"));
    assert_eq!(value_after(&cmd.args, "--user"), Some("Steve"));
}

fn make_zip(path: &Path, files: &[(&str, &str)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, content) in files {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn natives_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("natives.jar");
    make_zip(
        &jar,
        &[
            ("lwjgl.dll", "dll"),
            ("sub/other.so", "so"),
            ("META-INF/MANIFEST.MF", "m"),
            ("skip/me.txt", "x"),
        ],
    );
    let mut inst = installed(MODERN);
    inst.natives = vec![NativeJar { path: jar, exclude: vec!["skip/".into()] }];
    let natives = dir.path().join("natives");
    std::fs::create_dir_all(&natives).unwrap();
    std::fs::write(natives.join("stale.txt"), "old").unwrap();

    extract_natives(&inst, &natives).unwrap();
    assert_eq!(std::fs::read_to_string(natives.join("lwjgl.dll")).unwrap(), "dll");
    assert_eq!(std::fs::read_to_string(natives.join("sub/other.so")).unwrap(), "so");
    assert!(!natives.join("META-INF").exists());
    assert!(!natives.join("skip").exists());
    assert!(!natives.join("stale.txt").exists());
}

#[derive(Default)]
struct Collector(Mutex<Vec<Event>>);

impl EventSink for Collector {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

fn shell(script: &str) -> LaunchCommand {
    let (program, args) = if cfg!(windows) {
        ("cmd", vec!["/c".to_string(), script.to_string()])
    } else {
        ("sh", vec!["-c".to_string(), script.to_string()])
    };
    LaunchCommand {
        program: program.into(),
        args,
        cwd: std::env::temp_dir(),
        secrets: vec![],
    }
}

#[tokio::test]
async fn process_output_and_exit() {
    let manager = GameManager::new();
    let sink = Arc::new(Collector::default());
    let mut cmd = shell(&format!("echo hello && echo token={TOKEN} && exit 3"));
    cmd.secrets = vec![TOKEN.to_string()];
    let handle = manager.launch(cmd, "a", sink.clone()).await.unwrap();
    let exit = handle.wait().await;

    assert_eq!(exit.exit_code, Some(3));
    assert!(exit.ended_at >= exit.started_at);
    assert!(!manager.is_running("a"));
    assert_eq!(manager.last_exit("a").unwrap().exit_code, Some(3));

    let events = sink.0.lock().unwrap().clone();
    assert!(matches!(events.first(), Some(Event::GameState { running: true, .. })));
    assert!(matches!(events.last(), Some(Event::GameState { running: false, exit_code: Some(3), .. })));
    let logs: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::GameLog { line, stream, .. } => Some((line.trim().to_string(), *stream)),
            _ => None,
        })
        .collect();
    assert!(logs.iter().any(|(l, s)| l == "hello" && matches!(s, LogStream::Stdout)));
    assert!(logs.iter().any(|(l, _)| l == "token=<MASKED>"), "{logs:?}");
    assert!(!logs.iter().any(|(l, _)| l.contains(TOKEN)));

    let buffered = manager.log_lines("a");
    assert_eq!(buffered.len(), logs.len());
    assert!(buffered.iter().all(|l| !l.line.contains(TOKEN)));
}

#[tokio::test]
async fn kill_running_game() {
    let manager = GameManager::new();
    let sink = Arc::new(Collector::default());
    let sleeper = if cfg!(windows) { "ping -n 30 127.0.0.1 >nul" } else { "sleep 30" };
    let handle = manager.launch(shell(sleeper), "b", sink.clone()).await.unwrap();
    assert!(manager.is_running("b"));
    assert_eq!(manager.running().len(), 1);
    assert_eq!(manager.running()[0].instance_id, "b");
    assert!(manager
        .launch(shell(sleeper), "b", sink.clone())
        .await
        .is_err());

    manager.kill("b").unwrap();
    let exit = tokio::time::timeout(std::time::Duration::from_secs(10), handle.wait()).await.unwrap();
    assert!(exit.duration.as_secs() < 20);
    assert!(!manager.is_running("b"));
    assert!(manager.kill("b").is_err());
}

#[tokio::test]
async fn ring_buffer_is_bounded() {
    let manager = GameManager::new();
    let sink = Arc::new(Collector::default());
    let script = if cfg!(windows) {
        "for /L %i in (1,1,5100) do @echo line%i".to_string()
    } else {
        "i=1; while [ $i -le 5100 ]; do echo line$i; i=$((i+1)); done".to_string()
    };
    manager.launch(shell(&script), "c", sink).await.unwrap().wait().await;
    let lines = manager.log_lines("c");
    assert_eq!(lines.len(), LOG_BUFFER_LINES);
    assert_eq!(lines.last().unwrap().line.trim(), "line5100");
    assert_eq!(lines[0].line.trim(), "line101");
}


#[test]
fn demo_flag_on_modern_and_legacy() {
    for json in [MODERN, LEGACY] {
        let inst = installed(json);
        let full = build_command(&inst, &creds(), &opts(), Path::new("C:/n")).unwrap();
        assert!(!full.args.contains(&"--demo".to_string()));
        let demo = build_command(&inst, &creds(), &LaunchOptions { demo: true, ..opts() }, Path::new("C:/n")).unwrap();
        assert_eq!(demo.args.iter().filter(|a| *a == "--demo").count(), 1);
    }
}
