//! Persistent account list (`accounts.json`).

use std::path::{Path, PathBuf};

use cubby_common::paths::Paths;
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::Account;

const VERSION: u32 = 1;

#[derive(Serialize)]
struct FileFormat<'a> {
    version: u32,
    default: Option<String>,
    accounts: &'a [Account],
}

/// What is read back: accounts stay raw JSON so the offline ones older versions saved can be dropped.
#[derive(Deserialize)]
struct StoredFile {
    version: u32,
    default: Option<String>,
    accounts: Vec<serde_json::Value>,
}

/// Older versions saved offline accounts (`"kind": {"type": "offline"}`). They can't play any more.
fn is_offline(account: &serde_json::Value) -> bool {
    account.pointer("/kind/type").and_then(serde_json::Value::as_str) == Some("offline")
}

/// In-memory account list backed by a JSON file. Mutating methods only change memory: call
/// [`AccountStore::save`] to persist.
#[derive(Debug, Clone)]
pub struct AccountStore {
    path: PathBuf,
    accounts: Vec<Account>,
    default: Option<String>,
}

impl AccountStore {
    /// Loads `paths.accounts_file()`; a missing file gives an empty store.
    pub fn load(paths: &Paths) -> Result<Self> {
        Self::load_from(paths.accounts_file())
    }

    /// Like [`AccountStore::load`] with an explicit file path.
    pub fn load_from(path: PathBuf) -> Result<Self> {
        let mut was_offline = false;
        let (accounts, default) = match std::fs::read(&path) {
            Ok(bytes) => {
                let f: StoredFile = serde_json::from_slice(&bytes).map_err(|e| Error::json(path.display().to_string(), e))?;
                if f.version > VERSION {
                    return Err(Error::Other(format!("{} was saved by a newer version of Cubby (format version {}). Update Cubby to use it.", path.display(), f.version)));
                }
                let accounts = f
                    .accounts
                    .into_iter()
                    .filter(|a| {
                        let offline = is_offline(a);
                        was_offline |= offline && a.get("id").and_then(serde_json::Value::as_str) == f.default.as_deref();
                        !offline
                    })
                    .map(serde_json::from_value)
                    .collect::<std::result::Result<Vec<Account>, _>>()
                    .map_err(|e| Error::json(path.display().to_string(), e))?;
                (accounts, f.default)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Vec::new(), None),
            Err(e) => return Err(Error::io(&path, e)),
        };
        let mut store = Self { path, accounts, default };
        if store.default.as_ref().is_some_and(|d| store.get(d).is_none()) {
            store.default = None;
        }
        // A dropped offline default leaves no default: the player picks a Microsoft account.
        if store.default.is_none() && !was_offline {
            store.default = store.accounts.first().map(|a| a.id.clone());
        }
        Ok(store)
    }

    /// Writes the file atomically (temporary file in the same folder, then rename).
    pub fn save(&self) -> Result<()> {
        let file = FileFormat { version: VERSION, default: self.default.clone(), accounts: &self.accounts };
        let json = serde_json::to_vec_pretty(&file).map_err(|e| Error::json("accounts", e))?;
        write_atomic(&self.path, &json)
    }

    /// All accounts, in insertion order.
    pub fn list(&self) -> &[Account] {
        &self.accounts
    }

    /// Account by profile id.
    pub fn get(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Adds an account, or replaces the one with the same id. The first account becomes the default.
    pub fn add_or_update(&mut self, account: Account) {
        if self.default.is_none() {
            self.default = Some(account.id.clone());
        }
        match self.accounts.iter_mut().find(|a| a.id == account.id) {
            Some(slot) => *slot = account,
            None => self.accounts.push(account),
        }
    }

    /// Removes an account and returns it. If it was the default, the first remaining account takes over.
    pub fn remove(&mut self, id: &str) -> Option<Account> {
        let pos = self.accounts.iter().position(|a| a.id == id)?;
        let removed = self.accounts.remove(pos);
        if self.default.as_deref() == Some(id) {
            self.default = self.accounts.first().map(|a| a.id.clone());
        }
        Some(removed)
    }

    /// Chooses the default account.
    pub fn set_default(&mut self, id: &str) -> Result<()> {
        if self.get(id).is_none() {
            return Err(Error::NotFound(format!("account {id}")));
        }
        self.default = Some(id.to_owned());
        Ok(())
    }

    /// The default account, if any.
    pub fn default_account(&self) -> Option<&Account> {
        self.default.as_deref().and_then(|id| self.get(id))
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, data).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccountKind;
    use chrono::Utc;

    fn ms(id: &str, name: &str) -> Account {
        Account {
            id: id.into(),
            username: name.into(),
            kind: AccountKind::Microsoft { msa_refresh_token: "r".into(), mc_access_token: "t".into(), mc_token_expires_at: Utc::now(), xuid: String::new(), skin_url: None },
        }
    }

    #[test]
    fn round_trip_and_default() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("data"));
        let mut s = AccountStore::load(&paths).unwrap();
        assert!(s.list().is_empty() && s.default_account().is_none());

        let (a, b) = (ms("a1", "Steve"), ms("b2", "Alex"));
        s.add_or_update(a.clone());
        s.add_or_update(b.clone());
        assert_eq!(s.default_account(), Some(&a));
        s.set_default(&b.id).unwrap();
        assert!(s.set_default("nope").is_err());
        s.save().unwrap();
        assert!(!dir.path().join("data").join("accounts.json.tmp").exists());

        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(paths.accounts_file()).unwrap()).unwrap();
        assert_eq!(raw["version"], 1);
        assert_eq!(raw["default"], b.id.as_str());

        let mut s2 = AccountStore::load(&paths).unwrap();
        assert_eq!(s2.list(), [a.clone(), b.clone()]);
        assert_eq!(s2.default_account(), Some(&b));

        let mut renamed = a.clone();
        renamed.username = "Steve2".into();
        s2.add_or_update(renamed);
        assert_eq!(s2.list().len(), 2);
        assert_eq!(s2.get(&a.id).unwrap().username, "Steve2");

        assert!(s2.remove(&b.id).is_some());
        assert_eq!(s2.default_account().map(|x| x.id.as_str()), Some(a.id.as_str()));
        assert!(s2.remove(&b.id).is_none());
    }

    #[test]
    fn old_offline_accounts_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("accounts.json");
        let a = ms("a1", "Steve");
        let old = serde_json::json!({
            "version": 1,
            "default": "off",
            "accounts": [{ "id": "off", "username": "Guest", "kind": { "type": "offline" } }, a],
        });
        std::fs::write(&p, serde_json::to_vec(&old).unwrap()).unwrap();
        let s = AccountStore::load_from(p.clone()).unwrap();
        assert_eq!(s.list(), [a]);
        assert!(s.default_account().is_none());

        let only_offline = serde_json::json!({ "version": 1, "default": null, "accounts": [{ "id": "off", "username": "Guest", "kind": { "type": "offline" } }] });
        std::fs::write(&p, serde_json::to_vec(&only_offline).unwrap()).unwrap();
        let s = AccountStore::load_from(p).unwrap();
        assert!(s.list().is_empty() && s.default_account().is_none());
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("accounts.json");
        std::fs::write(&p, "{ nope").unwrap();
        assert!(AccountStore::load_from(p).is_err());
    }
}
