//! Account model and launch credentials.

use chrono::{DateTime, Duration, Utc};
use cubby_common::contracts::LaunchCredentials;
use serde::{Deserialize, Serialize};

/// A launcher account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Profile UUID, undashed lowercase hex (as returned by Mojang).
    pub id: String,
    /// In-game name.
    pub username: String,
    /// Account type and its secrets.
    pub kind: AccountKind,
}

/// Account type. Only Microsoft accounts that own Minecraft: Java Edition can be added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AccountKind {
    /// Signed-in Microsoft account owning Minecraft Java Edition.
    Microsoft {
        /// Long-lived Microsoft refresh token.
        msa_refresh_token: String,
        /// Minecraft services access token.
        mc_access_token: String,
        /// When `mc_access_token` expires.
        mc_token_expires_at: DateTime<Utc>,
        /// Xbox user id, empty when unknown.
        xuid: String,
        /// URL of the active skin texture.
        skin_url: Option<String>,
    },
}

impl Account {
    /// True when the Minecraft token expires within 5 minutes.
    pub fn needs_refresh(&self) -> bool {
        let AccountKind::Microsoft { mc_token_expires_at, .. } = &self.kind;
        *mc_token_expires_at - Duration::minutes(5) <= Utc::now()
    }

    /// Identity to pass to the game. Refresh the account first if [`Account::needs_refresh`].
    pub fn launch_credentials(&self) -> LaunchCredentials {
        let AccountKind::Microsoft { mc_access_token, xuid, .. } = &self.kind;
        LaunchCredentials {
            username: self.username.clone(),
            uuid: self.id.clone(),
            access_token: mc_access_token.clone(),
            user_type: "msa".into(),
            xuid: xuid.clone(),
        }
    }

    /// The owner's name and profile id without a session, to play with no internet. Ownership of
    /// the game was checked when the account signed in.
    pub fn offline_credentials(&self) -> LaunchCredentials {
        LaunchCredentials {
            username: self.username.clone(),
            uuid: self.id.clone(),
            access_token: "0".into(),
            user_type: "legacy".into(),
            xuid: String::new(),
        }
    }
}

/// Identity for the Minecraft demo, the only thing Cubby starts without a signed-in account.
pub fn demo_credentials() -> LaunchCredentials {
    LaunchCredentials {
        username: "Player".into(),
        uuid: "00000000000000000000000000000000".into(),
        access_token: "0".into(),
        user_type: "legacy".into(),
        xuid: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(expires: DateTime<Utc>) -> Account {
        Account {
            id: "abc".into(),
            username: "Alex".into(),
            kind: AccountKind::Microsoft {
                msa_refresh_token: "r".into(),
                mc_access_token: "tok".into(),
                mc_token_expires_at: expires,
                xuid: "2535".into(),
                skin_url: None,
            },
        }
    }

    #[test]
    fn microsoft_credentials_and_refresh() {
        let a = ms(Utc::now() + Duration::hours(1));
        let c = a.launch_credentials();
        assert_eq!((c.access_token.as_str(), c.user_type.as_str(), c.xuid.as_str()), ("tok", "msa", "2535"));
        assert!(!a.needs_refresh());
        assert!(ms(Utc::now() + Duration::minutes(4)).needs_refresh());
        assert!(ms(Utc::now() - Duration::minutes(1)).needs_refresh());
    }

    #[test]
    fn offline_credentials_keep_the_owner() {
        let c = ms(Utc::now()).offline_credentials();
        assert_eq!((c.username.as_str(), c.uuid.as_str()), ("Alex", "abc"));
        assert_eq!((c.access_token.as_str(), c.user_type.as_str(), c.xuid.as_str()), ("0", "legacy", ""));
    }

    #[test]
    fn serde_shape() {
        let json = serde_json::to_value(ms(Utc::now())).unwrap();
        assert_eq!(json["kind"]["type"], "microsoft");
        assert!(json["kind"]["msaRefreshToken"].is_string());
    }
}
