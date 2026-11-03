use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use cubby_common::contracts::{InstalledVersion, JavaInstall, LaunchCredentials};
use cubby_common::version::{rules_allow, Argument, RuleEnv};
use cubby_common::{Error, Result};

/// Name of the launcher, given to the game as `${launcher_name}`.
pub const LAUNCHER_NAME: &str = "Cubby";
/// Version of the launcher, given to the game as `${launcher_version}`.
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Join-a-world-on-start request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickPlay {
    /// Folder name of a local world.
    Singleplayer(String),
    /// Server address, `host` or `host:port`.
    Multiplayer(String),
}

/// Everything the user can tune for one launch.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub instance_id: String,
    /// Working directory of the game (`.minecraft` of the instance).
    pub game_dir: PathBuf,
    pub java: JavaInstall,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    /// Extra JVM arguments, inserted after the version's own and before the main class.
    pub extra_jvm_args: Vec<String>,
    /// Initial window size `(width, height)`.
    pub window: Option<(u32, u32)>,
    pub fullscreen: bool,
    pub quick_play: Option<QuickPlay>,
    /// Starts the Minecraft demo, for players without a signed-in account that owns the game.
    pub demo: bool,
}

/// A fully resolved process to spawn.
#[derive(Clone)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Sensitive strings (the access token) masked in the game's output by the `GameManager`.
    pub secrets: Vec<String>,
}

impl fmt::Debug for LaunchCommand {
    // The arguments contain the access token: never print them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchCommand")
            .field("program", &self.program)
            .field("args", &format_args!("<{} arguments>", self.args.len()))
            .field("cwd", &self.cwd)
            .finish()
    }
}

#[cfg(windows)]
const CP_SEP: &str = ";";
#[cfg(not(windows))]
const CP_SEP: &str = ":";

type Placeholders = HashMap<&'static str, String>;

/// Builds the command line that starts the game.
///
/// `${library_directory}` is derived as `<assets_root>/../libraries` (the contract carries no libraries root).
pub fn build_command(
    installed: &InstalledVersion,
    creds: &LaunchCredentials,
    opts: &LaunchOptions,
    natives_dir: &Path,
) -> Result<LaunchCommand> {
    let version = &installed.version;
    let main_class = version
        .main_class
        .clone()
        .ok_or_else(|| Error::Launch(format!("Version {} has no main class, so it can't be started.", version.id)))?;

    let env = RuleEnv::current()
        .with_feature("has_custom_resolution", opts.window.is_some())
        .with_feature("is_demo_user", opts.demo)
        .with_feature("is_quick_play_realms", false)
        .with_feature("is_quick_play_singleplayer", matches!(opts.quick_play, Some(QuickPlay::Singleplayer(_))))
        .with_feature("is_quick_play_multiplayer", matches!(opts.quick_play, Some(QuickPlay::Multiplayer(_))))
        .with_feature("has_quick_plays_support", opts.quick_play.is_some());

    let classpath = installed
        .libraries
        .iter()
        .chain(std::iter::once(&installed.client_jar))
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(CP_SEP);

    let ph = placeholders(installed, creds, opts, natives_dir, &classpath);

    // JVM
    let mut jvm: Vec<String> = vec![format!("-Xms{}M", opts.min_memory_mb), format!("-Xmx{}M", opts.max_memory_mb)];
    let modern = version.arguments.as_ref();
    match modern {
        Some(a) => jvm.extend(expand(&a.jvm, &env, &ph)),
        None => {
            if cfg!(windows) {
                jvm.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
            }
        }
    }
    if !jvm.iter().any(|a| a.starts_with("-Djava.library.path=")) {
        jvm.push(format!("-Djava.library.path={}", natives_dir.display()));
    }
    if !jvm.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm.push("-cp".into());
        jvm.push(classpath.clone());
    }
    if let Some(logging) = &installed.logging_argument {
        jvm.push(substitute(logging, &ph));
    }
    jvm.extend(opts.extra_jvm_args.iter().cloned());

    // game
    let mut game: Vec<String> = match (modern, &version.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => expand(&a.game, &env, &ph),
        (_, Some(legacy)) => legacy.split_whitespace().map(|s| substitute(s, &ph)).collect(),
        (Some(_), None) => Vec::new(),
        (None, None) => return Err(Error::Launch(format!("Version {} has no game arguments, so it can't be started.", version.id))),
    };
    let has = |game: &[String], flag: &str| game.iter().any(|a| a == flag);

    if let Some((w, h)) = opts.window {
        if !has(&game, "--width") {
            game.extend(["--width".into(), w.to_string(), "--height".into(), h.to_string()]);
        }
    }
    // Versions before 1.13 list their arguments in one string, without the demo rule.
    if opts.demo && !has(&game, "--demo") {
        game.push("--demo".into());
    }
    if opts.fullscreen && !has(&game, "--fullscreen") {
        game.push("--fullscreen".into());
    }
    match &opts.quick_play {
        Some(QuickPlay::Multiplayer(addr)) if !has(&game, "--quickPlayMultiplayer") => {
            let (host, port) = split_address(addr);
            game.extend(["--server".into(), host, "--port".into(), port]);
        }
        Some(QuickPlay::Singleplayer(_)) if !has(&game, "--quickPlaySingleplayer") => {
            tracing::warn!("this version cannot join a world directly; ignoring quick play");
        }
        _ => {}
    }

    let mut args = jvm;
    args.push(main_class);
    args.extend(game);

    let mut secrets = Vec::new();
    if !creds.access_token.is_empty() {
        secrets.push(creds.access_token.clone());
    }
    Ok(LaunchCommand { program: opts.java.executable.clone(), args, cwd: opts.game_dir.clone(), secrets })
}

fn split_address(addr: &str) -> (String, String) {
    // IPv6 literals in brackets keep their colons.
    if let Some(rest) = addr.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').unwrap_or("25565");
            return (host.to_string(), port.to_string());
        }
    }
    match addr.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') && port.parse::<u16>().is_ok() => (host.into(), port.into()),
        _ => (addr.into(), "25565".into()),
    }
}

fn placeholders(
    installed: &InstalledVersion,
    creds: &LaunchCredentials,
    opts: &LaunchOptions,
    natives_dir: &Path,
    classpath: &str,
) -> Placeholders {
    let version = &installed.version;
    let library_dir = installed.assets_root.parent().map(|p| p.join("libraries")).unwrap_or_default();
    let (w, h) = opts.window.unwrap_or((854, 480));
    let (qp_single, qp_multi) = match &opts.quick_play {
        Some(QuickPlay::Singleplayer(n)) => (n.clone(), String::new()),
        Some(QuickPlay::Multiplayer(a)) => (String::new(), a.clone()),
        None => (String::new(), String::new()),
    };
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let mut m = Placeholders::new();
    m.insert("natives_directory", path(natives_dir));
    m.insert("launcher_name", LAUNCHER_NAME.into());
    m.insert("launcher_version", LAUNCHER_VERSION.into());
    m.insert("classpath", classpath.into());
    m.insert("classpath_separator", CP_SEP.into());
    m.insert("library_directory", path(&library_dir));
    m.insert("version_name", version.id.clone());
    m.insert("auth_player_name", creds.username.clone());
    m.insert("game_directory", path(&opts.game_dir));
    m.insert("assets_root", path(&installed.assets_root));
    m.insert("assets_index_name", installed.assets_index_id.clone());
    m.insert("game_assets", path(&installed.game_assets_dir));
    m.insert("auth_uuid", creds.uuid.clone());
    m.insert("auth_access_token", creds.access_token.clone());
    m.insert("auth_session", format!("token:{}:{}", creds.access_token, creds.uuid));
    m.insert("clientid", String::new());
    m.insert("auth_xuid", creds.xuid.clone());
    m.insert("user_type", creds.user_type.clone());
    m.insert("version_type", version.kind.clone().unwrap_or_else(|| "release".into()));
    m.insert("user_properties", "{}".into());
    m.insert("resolution_width", w.to_string());
    m.insert("resolution_height", h.to_string());
    m.insert("quickPlayPath", "quickPlay/java/cubby.json".into());
    m.insert("quickPlaySingleplayer", qp_single);
    m.insert("quickPlayMultiplayer", qp_multi);
    m.insert("quickPlayRealms", String::new());
    m
}

/// Filters conditional arguments by rules and substitutes placeholders.
fn expand(args: &[Argument], env: &RuleEnv, ph: &Placeholders) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(substitute(s, ph)),
            Argument::Conditional { rules, value } => {
                if rules_allow(rules, env) {
                    out.extend(value.values().into_iter().map(|v| substitute(v, ph)));
                }
            }
        }
    }
    out
}

/// Replaces `${name}`; unknown names are left untouched (only the name is logged, never values).
fn substitute(input: &str, ph: &Placeholders) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                match ph.get(name) {
                    Some(v) => out.push_str(v),
                    None => {
                        tracing::warn!("unknown launch placeholder: {name}");
                        out.push_str(&rest[start..start + 3 + end]);
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
