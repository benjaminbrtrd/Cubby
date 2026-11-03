//! Microsoft accounts that own Minecraft: Java Edition.
//!
//! Microsoft sign-in uses the device code flow, then Xbox Live, XSTS and the Minecraft services.
//! It needs an Azure app id approved by Mojang for the Minecraft API, built in or read from
//! `CUBBY_MSA_CLIENT_ID`.

pub mod account;
pub mod msa;
pub mod store;
pub mod xbox;

pub use account::{demo_credentials, Account, AccountKind};
pub use msa::{finish_device_login, refresh, start_device_login, DeviceCode};
pub use store::AccountStore;

/// Cancellation signal for [`finish_device_login`]: send `true` (or drop the sender) to abort.
pub type Cancel = tokio::sync::watch::Receiver<bool>;

/// Cubby's own Azure application (client) id, once it is registered and approved by Mojang. It is
/// not a secret: every copy of the app sends it to Microsoft. Empty = this build has none.
pub const BUILT_IN_CLIENT_ID: &str = "";

/// Where the client id returned by [`client_id`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIdSource {
    /// `CUBBY_MSA_CLIENT_ID` set when the app runs.
    Environment,
    /// Inside the app: [`BUILT_IN_CLIENT_ID`], or the same variable set when it was compiled.
    BuiltIn,
}

fn clean(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// The Azure application (client) id used for Microsoft sign-in, and where it comes from: the
/// environment when the app runs, else the one compiled in, else `None`. Empty values are ignored.
pub fn client_id_with_source() -> Option<(String, ClientIdSource)> {
    let runtime = std::env::var("CUBBY_MSA_CLIENT_ID").ok().as_deref().and_then(clean);
    if let Some(id) = runtime {
        return Some((id, ClientIdSource::Environment));
    }
    [option_env!("CUBBY_MSA_CLIENT_ID"), Some(BUILT_IN_CLIENT_ID)]
        .into_iter()
        .flatten()
        .find_map(clean)
        .map(|id| (id, ClientIdSource::BuiltIn))
}

/// The Azure application (client) id used for Microsoft sign-in, if this build or its environment has one.
pub fn client_id() -> Option<String> {
    client_id_with_source().map(|(id, _)| id)
}

/// True when the Minecraft token of `account` expires within 5 minutes.
pub fn needs_refresh(account: &Account) -> bool {
    account.needs_refresh()
}
