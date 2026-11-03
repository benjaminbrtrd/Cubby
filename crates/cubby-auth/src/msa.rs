//! Microsoft identity platform: device code flow and token refresh.

use std::time::Duration;

use cubby_common::net::Http;
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::xbox::{self, McSession};
use crate::{Account, AccountKind, Cancel};

/// Device code endpoint.
pub const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
/// Token endpoint.
pub const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
/// OAuth scope requested.
pub const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const DEFAULT_INTERVAL: u64 = 5;

/// What the user must do to sign in; shown by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCode {
    /// Code to type on the verification page.
    pub user_code: String,
    /// Page where the user enters the code (`https://www.microsoft.com/link`).
    pub verification_uri: String,
    /// Opaque code used for polling.
    pub device_code: String,
    /// Seconds before the code expires.
    pub expires_in: u64,
    /// Polling interval in seconds.
    #[serde(default = "default_interval")]
    pub interval: u64,
    /// Ready-to-display instruction from Microsoft.
    #[serde(default)]
    pub message: String,
}

fn default_interval() -> u64 {
    DEFAULT_INTERVAL
}

/// Tokens obtained from the token endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsaTokens {
    /// Access token (for Xbox Live).
    pub access_token: String,
    /// Refresh token; absent in some refresh responses.
    pub refresh_token: Option<String>,
}

/// Outcome of one poll of the token endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPoll {
    /// The user has not finished yet.
    Pending,
    /// Poll less often: add 5 seconds to the interval.
    SlowDown,
    /// Signed in.
    Done(MsaTokens),
}


/// Form fields of the device code request.
pub fn device_code_form(client_id: &str) -> Vec<(&'static str, String)> {
    vec![("client_id", client_id.to_owned()), ("scope", SCOPE.to_owned())]
}

/// Form fields of one token poll.
pub fn poll_form(client_id: &str, device_code: &str) -> Vec<(&'static str, String)> {
    vec![("client_id", client_id.to_owned()), ("grant_type", DEVICE_GRANT.to_owned()), ("device_code", device_code.to_owned())]
}

/// Form fields of a refresh-token request.
pub fn refresh_form(client_id: &str, refresh_token: &str) -> Vec<(&'static str, String)> {
    vec![
        ("client_id", client_id.to_owned()),
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", refresh_token.to_owned()),
        ("scope", SCOPE.to_owned()),
    ]
}

#[derive(Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

fn oauth_error(body: &[u8]) -> Option<OAuthError> {
    serde_json::from_slice(body).ok()
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

/// Parses the device code response.
pub fn parse_device_code(status: u16, body: &[u8]) -> Result<DeviceCode> {
    if (200..300).contains(&status) {
        return serde_json::from_slice(body).map_err(|e| Error::Auth(format!("Couldn't read Microsoft's sign-in response ({e}). Try again.")));
    }
    Err(Error::Auth(match oauth_error(body) {
        Some(e) => format!("Microsoft refused the sign-in request: {} ({}).", first_line(e.error_description.as_deref().unwrap_or("")), e.error),
        None => format!("Microsoft refused the sign-in request (HTTP {status}). Try again later."),
    }))
}

/// Parses a token endpoint response (device code polling or refresh).
pub fn parse_token_response(status: u16, body: &[u8]) -> Result<TokenPoll> {
    if (200..300).contains(&status) {
        #[derive(Deserialize)]
        struct Ok {
            access_token: String,
            #[serde(default)]
            refresh_token: Option<String>,
        }
        let t: Ok = serde_json::from_slice(body).map_err(|e| Error::Auth(format!("Couldn't read Microsoft's sign-in response ({e}). Try again.")))?;
        return Result::Ok(TokenPoll::Done(MsaTokens { access_token: t.access_token, refresh_token: t.refresh_token }));
    }
    let Some(e) = oauth_error(body) else {
        return Err(Error::Auth(format!("Microsoft's sign-in service returned HTTP {status}. Try again later.")));
    };
    match e.error.as_str() {
        "authorization_pending" => Ok(TokenPoll::Pending),
        "slow_down" => Ok(TokenPoll::SlowDown),
        "expired_token" => Err(Error::Auth("The sign-in code expired before it was used. Start again.".into())),
        "authorization_declined" => Err(Error::Auth("The sign-in was declined.".into())),
        "bad_verification_code" => Err(Error::Auth("The sign-in code isn't valid. Start again.".into())),
        "invalid_grant" => Err(Error::Auth("Your Microsoft session expired or was revoked. Sign in again.".into())),
        other => Err(Error::Auth(format!("Microsoft sign-in failed: {} ({other}).", first_line(e.error_description.as_deref().unwrap_or("")))))
    }
}


async fn post_form(http: &Http, url: &str, form: &[(&str, String)]) -> Result<(u16, Vec<u8>)> {
    let resp = http.client().post(url).header("Accept", "application/json").form(form).send().await.map_err(|e| Error::http(url, e))?;
    let status = resp.status().as_u16();
    let body = resp.bytes().await.map_err(|e| Error::http(url, e))?;
    Ok((status, body.to_vec()))
}

/// Starts the device code flow. Show `user_code` and `verification_uri` to the user, then call
/// [`finish_device_login`].
pub async fn start_device_login(http: &Http, client_id: &str) -> Result<DeviceCode> {
    let (status, body) = post_form(http, DEVICE_CODE_URL, &device_code_form(client_id)).await?;
    parse_device_code(status, &body)
}

async fn cancelled(cancel: &mut Cancel) {
    loop {
        if *cancel.borrow() || cancel.changed().await.is_err() {
            return;
        }
    }
}

/// Runs `fut` unless cancellation is signalled first (`true` sent, or the sender dropped).
async fn with_cancel<T>(cancel: &mut Cancel, fut: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::select! {
        biased;
        _ = cancelled(cancel) => Err(Error::Cancelled),
        r = fut => r,
    }
}

/// Polls the token endpoint until the user has signed in, then runs the Xbox/Minecraft chain and
/// returns the account. Cancel by sending `true` on the watch channel (or dropping its sender): the
/// result is then `Error::Cancelled`.
pub async fn finish_device_login(http: &Http, client_id: &str, code: &DeviceCode, mut cancel: Cancel) -> Result<Account> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = code.interval.max(1);
    let form = poll_form(client_id, &code.device_code);
    let tokens = loop {
        with_cancel(&mut cancel, async {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            Ok(())
        })
        .await?;
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Auth("The sign-in code expired before it was used. Start again.".into()));
        }
        let (status, body) = with_cancel(&mut cancel, post_form(http, TOKEN_URL, &form)).await?;
        match parse_token_response(status, &body)? {
            TokenPoll::Pending => {}
            TokenPoll::SlowDown => interval += 5,
            TokenPoll::Done(t) => break t,
        }
    };
    let refresh_token = tokens.refresh_token.ok_or_else(|| Error::Auth("Microsoft didn't return a refresh token. Try again.".into()))?;
    let session = with_cancel(&mut cancel, xbox::minecraft_session(http, &tokens.access_token)).await?;
    Ok(account_from_session(session, refresh_token))
}

fn account_from_session(session: McSession, msa_refresh_token: String) -> Account {
    let skin_url = session.profile.active_skin_url();
    Account {
        id: session.profile.id,
        username: session.profile.name,
        kind: AccountKind::Microsoft {
            msa_refresh_token,
            mc_access_token: session.token.access_token,
            mc_token_expires_at: session.expires_at,
            xuid: session.xuid,
            skin_url,
        },
    }
}

/// Renews the Minecraft token of a Microsoft account when it expires within 5 minutes (see
/// [`Account::needs_refresh`]); does nothing otherwise. On failure the
/// account is left untouched; an `Error::Auth` means the user must sign in again.
pub async fn refresh(http: &Http, client_id: &str, account: &mut Account) -> Result<()> {
    if !account.needs_refresh() {
        return Ok(());
    }
    let AccountKind::Microsoft { msa_refresh_token, .. } = &account.kind;
    let (status, body) = post_form(http, TOKEN_URL, &refresh_form(client_id, msa_refresh_token)).await?;
    let tokens = match parse_token_response(status, &body)? {
        TokenPoll::Done(t) => t,
        _ => return Err(Error::Auth("Microsoft sent an unexpected response while renewing your session. Try again.".into())),
    };
    let new_refresh = tokens.refresh_token.unwrap_or_else(|| msa_refresh_token.clone());
    let session = xbox::minecraft_session(http, &tokens.access_token).await?;
    *account = account_from_session(session, new_refresh);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(form: &'a [(&str, String)], k: &str) -> &'a str {
        &form.iter().find(|(key, _)| *key == k).unwrap().1
    }

    #[test]
    fn forms() {
        let f = device_code_form("cid");
        assert_eq!((get(&f, "client_id"), get(&f, "scope")), ("cid", "XboxLive.signin offline_access"));
        let f = poll_form("cid", "dc");
        assert_eq!(get(&f, "grant_type"), "urn:ietf:params:oauth:grant-type:device_code");
        assert_eq!(get(&f, "device_code"), "dc");
        let f = refresh_form("cid", "rt");
        assert_eq!((get(&f, "grant_type"), get(&f, "refresh_token")), ("refresh_token", "rt"));
    }

    #[test]
    fn device_code_parsing() {
        let body = br#"{"user_code":"ABCD1234","device_code":"DAQABAAEA","verification_uri":"https://www.microsoft.com/link","expires_in":900,"interval":5,"message":"To sign in, use a web browser to open the page https://www.microsoft.com/link and enter the code ABCD1234 to authenticate."}"#;
        let d = parse_device_code(200, body).unwrap();
        assert_eq!((d.user_code.as_str(), d.expires_in, d.interval), ("ABCD1234", 900, 5));
        assert!(d.message.contains("ABCD1234"));
        // interval and message are optional
        let d = parse_device_code(200, br#"{"user_code":"U","device_code":"D","verification_uri":"v","expires_in":1}"#).unwrap();
        assert_eq!(d.interval, 5);
        let err = parse_device_code(400, br#"{"error":"invalid_client","error_description":"AADSTS700016: Application not found.\r\nTrace ID: x"}"#).unwrap_err().to_string();
        assert!(err.contains("AADSTS700016") && !err.contains("Trace"), "{err}");
        assert!(parse_device_code(500, b"<html>").is_err());
    }

    #[test]
    fn token_parsing() {
        let ok = br#"{"token_type":"Bearer","scope":"XboxLive.signin","expires_in":3599,"access_token":"AT","refresh_token":"RT"}"#;
        assert_eq!(parse_token_response(200, ok).unwrap(), TokenPoll::Done(MsaTokens { access_token: "AT".into(), refresh_token: Some("RT".into()) }));
        assert_eq!(parse_token_response(400, br#"{"error":"authorization_pending"}"#).unwrap(), TokenPoll::Pending);
        assert_eq!(parse_token_response(400, br#"{"error":"slow_down"}"#).unwrap(), TokenPoll::SlowDown);
        for (code, needle) in [("expired_token", "expired"), ("authorization_declined", "declined"), ("invalid_grant", "Sign in again"), ("weird", "weird")] {
            let body = format!(r#"{{"error":"{code}","error_description":"d"}}"#);
            let err = parse_token_response(400, body.as_bytes()).unwrap_err().to_string();
            assert!(err.contains(needle), "{code}: {err}");
        }
        assert!(parse_token_response(502, b"bad gateway").is_err());
    }

    #[test]
    fn device_code_serializes_for_the_ui() {
        let d = parse_device_code(200, br#"{"user_code":"U","device_code":"D","verification_uri":"v","expires_in":1,"interval":5}"#).unwrap();
        let back: DeviceCode = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
        assert_eq!(d, back);
    }

    #[tokio::test]
    async fn cancellation_is_honoured() {
        let http = Http::new().unwrap();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let code = DeviceCode { user_code: "U".into(), verification_uri: "v".into(), device_code: "D".into(), expires_in: 900, interval: 5, message: String::new() };
        tx.send(true).unwrap();
        let err = finish_device_login(&http, "cid", &code, rx).await.unwrap_err();
        assert!(matches!(err, Error::Cancelled));
    }
}
