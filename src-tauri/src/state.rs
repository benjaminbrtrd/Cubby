use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use cubby_auth::{AccountStore, DeviceCode};
use cubby_common::net::Http;
use cubby_common::paths::Paths;
use cubby_instance::{InstanceStore, Settings};
use cubby_launch::GameManager;
use cubby_atlauncher::AtLauncherClient;
use cubby_curseforge::{ApiKeySource, CurseForgeClient};
use cubby_ftb::FtbClient;
use cubby_modrinth::Modrinth;
use cubby_technic::TechnicClient;
use tauri::AppHandle;
use tokio::sync::watch;

use crate::events::TauriSink;

/// A Microsoft login waiting for the user to enter the code.
pub struct PendingLogin {
    pub code: DeviceCode,
    pub cancel: watch::Sender<bool>,
}

pub struct Core {
    pub paths: Paths,
    pub http: Http,
    pub store: InstanceStore,
    pub accounts: tokio::sync::Mutex<AccountStore>,
    pub settings: Mutex<Settings>,
    pub games: GameManager,
    pub sink: Arc<TauriSink>,
    pub login: Mutex<Option<PendingLogin>>,
    pub modrinth: Modrinth,
    pub ftb: FtbClient,
    pub technic: TechnicClient,
    pub atlauncher: AtLauncherClient,
    /// Ids of the background tasks running (`install:<id>`, `modpack:<id>`...), one of each at a time.
    pub tasks: Mutex<HashSet<String>>,
}

impl Core {
    pub fn new(app: AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let paths = Paths::from_env()?;
        paths.ensure()?;
        let settings = Settings::load(&paths).unwrap_or_else(|e| {
            log::warn!("settings unreadable, using defaults: {e}");
            Settings::default()
        });
        // An unreadable accounts file is kept aside rather than overwritten: accounts go to a new file.
        let accounts = match AccountStore::load(&paths) {
            Ok(store) => store,
            Err(e) => {
                log::warn!("accounts unreadable, starting from accounts.recovered.json: {e}");
                AccountStore::load_from(paths.root().join("accounts.recovered.json"))?
            }
        };
        let http = Http::with_proxy(&proxy_config(&settings.proxy))?;
        Ok(Self {
            sink: Arc::new(TauriSink::new(app)),
            modrinth: Modrinth::new(http.clone()),
            ftb: FtbClient::new(http.clone()),
            technic: TechnicClient::new(http.clone()),
            atlauncher: AtLauncherClient::new(http.clone()),
            http,
            store: InstanceStore::new(paths.clone()),
            accounts: tokio::sync::Mutex::new(accounts),
            settings: Mutex::new(settings),
            games: GameManager::new(),
            login: Mutex::new(None),
            tasks: Mutex::new(HashSet::new()),
            paths,
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().expect("settings lock").clone()
    }

    /// Azure client id: the one in the settings (an advanced override), else the environment's or the
    /// one built into the app.
    pub fn msa_client_id(&self) -> Option<String> {
        self.msa_client_id_with_source().map(|(id, _)| id)
    }

    /// The client id and where it comes from: "settings", "environment" or "builtIn".
    pub fn msa_client_id_with_source(&self) -> Option<(String, &'static str)> {
        if let Some(id) = self.settings().msa_client_id.map(|id| id.trim().to_owned()).filter(|id| !id.is_empty()) {
            return Some((id, "settings"));
        }
        cubby_auth::client_id_with_source().map(|(id, source)| {
            (id, match source {
                cubby_auth::ClientIdSource::Environment => "environment",
                cubby_auth::ClientIdSource::BuiltIn => "builtIn",
            })
        })
    }
}

impl Core {
    /// CurseForge API key and where it comes from ("settings", "environment" or "builtIn"): the one in the
    /// settings, else `CUBBY_CURSEFORGE_API_KEY` at run time, else the one compiled in.
    pub fn curseforge_key_with_source(&self) -> Option<(String, &'static str)> {
        if let Some(key) = self.settings().curseforge_api_key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()) {
            return Some((key, "settings"));
        }
        cubby_curseforge::api_key_with_source().map(|(key, source)| {
            (key, match source {
                ApiKeySource::Environment => "environment",
                ApiKeySource::BuiltIn => "builtIn",
            })
        })
    }

    /// A CurseForge client with the current key (cheap, so built per call: a key saved in the settings
    /// applies right away), `None` without a key.
    pub fn curseforge(&self) -> Option<CurseForgeClient> {
        self.curseforge_key_with_source().map(|(key, _)| CurseForgeClient::new(self.http.clone(), key))
    }
}

/// Proxy for every HTTP request, from the settings (read once, at start).
fn proxy_config(proxy: &cubby_instance::ProxySettings) -> cubby_common::net::ProxyConfig {
    use cubby_common::net::ProxyConfig;
    match proxy.mode {
        cubby_instance::ProxyMode::System => ProxyConfig::System,
        cubby_instance::ProxyMode::None => ProxyConfig::None,
        cubby_instance::ProxyMode::Manual if !proxy.url.trim().is_empty() => ProxyConfig::Manual(proxy.url.trim().to_string()),
        cubby_instance::ProxyMode::Manual => ProxyConfig::System,
    }
}
