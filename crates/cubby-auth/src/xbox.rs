//! Xbox Live, XSTS and Minecraft services steps: pure request builders / response parsers, plus the
//! thin network glue [`minecraft_session`].

use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use cubby_common::net::Http;
use cubby_common::{Error, Result};
use serde::Deserialize;
use serde_json::{json, Value};

/// Xbox Live user authentication endpoint.
pub const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
/// XSTS authorization endpoint.
pub const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
/// Minecraft services Xbox login endpoint.
pub const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
/// Minecraft store entitlements endpoint.
pub const ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
/// Minecraft profile endpoint.
pub const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
/// XSTS relying party of the Minecraft services.
pub const MC_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";

/// An Xbox Live token with its user hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XboxToken {
    /// JWT.
    pub token: String,
    /// User hash (`uhs` display claim).
    pub uhs: String,
    /// Xbox user id (`xid` display claim) when present.
    pub xuid: Option<String>,
}

/// Minecraft services access token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McToken {
    /// Bearer token for the Minecraft API and the game.
    pub access_token: String,
    /// Lifetime in seconds.
    pub expires_in: i64,
}

/// A skin of the profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Skin {
    /// Skin id.
    #[serde(default)]
    pub id: String,
    /// `ACTIVE` or `INACTIVE`.
    #[serde(default)]
    pub state: String,
    /// Texture URL.
    pub url: String,
    /// `CLASSIC` or `SLIM`.
    #[serde(default)]
    pub variant: String,
}

/// A cape of the profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Cape {
    /// Cape id.
    #[serde(default)]
    pub id: String,
    /// `ACTIVE` or `INACTIVE`.
    #[serde(default)]
    pub state: String,
    /// Texture URL.
    #[serde(default)]
    pub url: String,
    /// Display name.
    #[serde(default)]
    pub alias: String,
}

/// Minecraft profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    /// Undashed UUID.
    pub id: String,
    /// In-game name.
    pub name: String,
    /// Skins.
    #[serde(default)]
    pub skins: Vec<Skin>,
    /// Capes.
    #[serde(default)]
    pub capes: Vec<Cape>,
}

impl Profile {
    /// URL of the active skin.
    pub fn active_skin_url(&self) -> Option<String> {
        self.skins.iter().find(|s| s.state.eq_ignore_ascii_case("ACTIVE")).map(|s| s.url.clone())
    }
}

/// Everything obtained from the Xbox/Minecraft chain.
#[derive(Debug, Clone)]
pub struct McSession {
    /// Minecraft access token.
    pub token: McToken,
    /// Absolute expiry of the token.
    pub expires_at: DateTime<Utc>,
    /// Xbox user id, empty when unknown.
    pub xuid: String,
    /// Profile.
    pub profile: Profile,
}

// request bodies

/// Body of the Xbox Live user authentication request.
pub fn xbl_request_body(msa_access_token: &str) -> Value {
    json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={msa_access_token}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    })
}

/// Body of the XSTS authorization request.
pub fn xsts_request_body(xbl_token: &str) -> Value {
    json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": MC_RELYING_PARTY,
        "TokenType": "JWT",
    })
}

/// Body of the `login_with_xbox` request.
pub fn mc_login_body(uhs: &str, xsts_token: &str) -> Value {
    json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") })
}

// response parsers

#[derive(Deserialize)]
struct XTokenResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    claims: Claims,
}

#[derive(Deserialize)]
struct Claims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

fn parse_xtoken(what: &str, body: &[u8]) -> Result<XboxToken> {
    let r: XTokenResponse = serde_json::from_slice(body).map_err(|e| Error::Auth(format!("Couldn't read the {what} response ({e}). Try again.")))?;
    let xui = r.claims.xui.into_iter().next().ok_or_else(|| Error::Auth(format!("The {what} response has no user hash. Try again.")))?;
    Ok(XboxToken { token: r.token, uhs: xui.uhs, xuid: xui.xid })
}

/// Parses the Xbox Live user authentication response.
pub fn parse_xbl_response(status: u16, body: &[u8]) -> Result<XboxToken> {
    if !(200..300).contains(&status) {
        return Err(Error::Auth(format!("Xbox Live refused the Microsoft sign-in (HTTP {status}).")));
    }
    parse_xtoken("Xbox Live", body)
}

/// Parses the XSTS response; maps `XErr` codes of error responses to readable messages.
pub fn parse_xsts_response(status: u16, body: &[u8]) -> Result<XboxToken> {
    if status == 401 {
        let code = serde_json::from_slice::<Value>(body).ok().and_then(|v| v.get("XErr").and_then(Value::as_u64));
        return Err(Error::Auth(match code {
            Some(code) => xerr_message(code),
            None => "Xbox services refused the sign-in (HTTP 401, no error code).".into(),
        }));
    }
    if !(200..300).contains(&status) {
        return Err(Error::Auth(format!("Xbox services returned HTTP {status}. Try again later.")));
    }
    parse_xtoken("Xbox services", body)
}

/// Human-readable explanation of an XSTS `XErr` code.
pub fn xerr_message(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account doesn't have an Xbox Live profile. Sign in once on minecraft.net or xbox.com to create one.".into(),
        2148916235 => "Xbox Live isn't available in your country.".into(),
        2148916236 | 2148916237 => "This Microsoft account needs adult verification (proof of age) on Xbox. Sign in on login.live.com to complete it.".into(),
        2148916238 => "This Microsoft account belongs to a child and isn't part of a family group. Add it to a Microsoft family group first (help.minecraft.net/hc/en-us/articles/4408968616077).".into(),
        other => format!("Xbox services refused the sign-in (unknown error {other})."),
    }
}

#[derive(Deserialize)]
struct McLoginResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<i64>,
}

/// Parses the `login_with_xbox` response. 403 means the Azure application is not approved by Mojang.
pub fn parse_mc_login_response(status: u16, body: &[u8]) -> Result<McToken> {
    match status {
        403 => Err(Error::Auth(
            "Minecraft services refused the sign-in (HTTP 403). This launcher's Azure application hasn't been approved by Mojang yet (https://aka.ms/mce-reviewappid).".into(),
        )),
        s if !(200..300).contains(&s) => Err(Error::Auth(format!("Minecraft services refused the sign-in (HTTP {s}). Try again later."))),
        _ => {
            let r: McLoginResponse = serde_json::from_slice(body).map_err(|e| Error::Auth(format!("Couldn't read the Minecraft sign-in response ({e}). Try again.")))?;
            Ok(McToken { access_token: r.access_token, expires_in: r.expires_in.unwrap_or(86400) })
        }
    }
}

/// Parses the entitlements response: true when the account has Minecraft Java Edition.
pub fn parse_entitlements(status: u16, body: &[u8]) -> Result<bool> {
    if !(200..300).contains(&status) {
        return Err(Error::Auth(format!("Couldn't check whether this account owns Minecraft (HTTP {status}). Try again later.")));
    }
    let v: Value = serde_json::from_slice(body).map_err(|e| Error::Auth(format!("Couldn't read the Minecraft ownership response ({e}). Try again.")))?;
    Ok(v.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|i| matches!(i.get("name").and_then(Value::as_str), Some("game_minecraft" | "product_minecraft")))))
}

const NOT_OWNED: &str = "This Microsoft account doesn't own Minecraft Java Edition.";

/// Parses the profile response; 404 means the account does not own the game.
pub fn parse_profile(status: u16, body: &[u8]) -> Result<Profile> {
    match status {
        404 => Err(Error::Auth(NOT_OWNED.into())),
        s if !(200..300).contains(&s) => Err(Error::Auth(format!("Couldn't load the Minecraft profile (HTTP {s}). Try again later."))),
        _ => serde_json::from_slice(body).map_err(|e| Error::Auth(format!("Couldn't read the Minecraft profile ({e}). Try again."))),
    }
}

/// Reads a string claim from the payload of a JWT (no signature check; used for the `xuid` hint only).
pub fn jwt_claim(jwt: &str, claim: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    match v.get(claim)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}


async fn send(req: reqwest::RequestBuilder, url: &str) -> Result<(u16, Vec<u8>)> {
    let resp = req.send().await.map_err(|e| Error::http(url, e))?;
    let status = resp.status().as_u16();
    let body = resp.bytes().await.map_err(|e| Error::http(url, e))?;
    Ok((status, body.to_vec()))
}

async fn post_json(http: &Http, url: &str, body: &Value) -> Result<(u16, Vec<u8>)> {
    send(http.client().post(url).header("Accept", "application/json").header("x-xbl-contract-version", "1").json(body), url).await
}

async fn get_bearer(http: &Http, url: &str, token: &str) -> Result<(u16, Vec<u8>)> {
    send(http.client().get(url).bearer_auth(token).header("Accept", "application/json"), url).await
}

/// Runs the whole chain from a Microsoft access token: Xbox Live, XSTS, `login_with_xbox`,
/// entitlements and profile.
pub async fn minecraft_session(http: &Http, msa_access_token: &str) -> Result<McSession> {
    let (s, b) = post_json(http, XBL_URL, &xbl_request_body(msa_access_token)).await?;
    let xbl = parse_xbl_response(s, &b)?;

    let (s, b) = post_json(http, XSTS_URL, &xsts_request_body(&xbl.token)).await?;
    let xsts = parse_xsts_response(s, &b)?;

    let (s, b) = post_json(http, MC_LOGIN_URL, &mc_login_body(&xsts.uhs, &xsts.token)).await?;
    let token = parse_mc_login_response(s, &b)?;
    let expires_at = Utc::now() + Duration::seconds(token.expires_in);

    let (s, b) = get_bearer(http, ENTITLEMENTS_URL, &token.access_token).await?;
    if !parse_entitlements(s, &b)? {
        return Err(Error::Auth(NOT_OWNED.into()));
    }

    let (s, b) = get_bearer(http, PROFILE_URL, &token.access_token).await?;
    let profile = parse_profile(s, &b)?;

    let xuid = xsts.xuid.or_else(|| jwt_claim(&token.access_token, "xuid")).unwrap_or_default();
    Ok(McSession { token, expires_at, xuid, profile })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XSTS_OK: &str = r#"{"IssueInstant":"2024-01-01T00:00:00.0000000Z","NotAfter":"2024-01-02T00:00:00.0000000Z","Token":"xsts-token","DisplayClaims":{"xui":[{"uhs":"1234567890"}]}}"#;

    #[test]
    fn request_bodies() {
        let b = xbl_request_body("MSA");
        assert_eq!(b["Properties"]["RpsTicket"], "d=MSA");
        assert_eq!(b["Properties"]["AuthMethod"], "RPS");
        assert_eq!(b["Properties"]["SiteName"], "user.auth.xboxlive.com");
        assert_eq!(b["RelyingParty"], "http://auth.xboxlive.com");
        assert_eq!(b["TokenType"], "JWT");

        let b = xsts_request_body("XBL");
        assert_eq!(b["Properties"]["SandboxId"], "RETAIL");
        assert_eq!(b["Properties"]["UserTokens"][0], "XBL");
        assert_eq!(b["RelyingParty"], "rp://api.minecraftservices.com/");

        assert_eq!(mc_login_body("uhs1", "tok")["identityToken"], "XBL3.0 x=uhs1;tok");
    }

    #[test]
    fn xbox_token_parsing() {
        let t = parse_xbl_response(200, XSTS_OK.as_bytes()).unwrap();
        assert_eq!((t.token.as_str(), t.uhs.as_str(), t.xuid), ("xsts-token", "1234567890", None));
        let with_xid = br#"{"Token":"t","DisplayClaims":{"xui":[{"uhs":"u","xid":"2535"}]}}"#;
        assert_eq!(parse_xsts_response(200, with_xid).unwrap().xuid.as_deref(), Some("2535"));
        assert!(parse_xbl_response(400, b"{}").is_err());
        assert!(parse_xbl_response(200, b"{}").is_err());
        assert!(parse_xbl_response(200, br#"{"Token":"t","DisplayClaims":{"xui":[]}}"#).is_err());
    }

    #[test]
    fn xsts_errors() {
        for (code, needle) in [
            (2148916233u64, "Xbox Live profile"),
            (2148916235, "available in your country"),
            (2148916236, "proof of age"),
            (2148916237, "proof of age"),
            (2148916238, "family"),
        ] {
            let body = format!(r#"{{"Identity":"0","XErr":{code},"Message":"","Redirect":"https://x"}}"#);
            let err = parse_xsts_response(401, body.as_bytes()).unwrap_err().to_string();
            assert!(err.contains(needle), "{code}: {err}");
        }
        assert!(parse_xsts_response(401, br#"{"XErr":1}"#).unwrap_err().to_string().contains("unknown error 1"));
        assert!(parse_xsts_response(401, b"nope").is_err());
        assert!(parse_xsts_response(500, b"").is_err());
    }

    #[test]
    fn login_with_xbox() {
        let ok = br#"{"username":"uuid","roles":[],"access_token":"mc-token","token_type":"Bearer","expires_in":86400}"#;
        assert_eq!(parse_mc_login_response(200, ok).unwrap(), McToken { access_token: "mc-token".into(), expires_in: 86400 });
        let err = parse_mc_login_response(403, b"{}").unwrap_err().to_string();
        assert!(err.contains("been approved by Mojang"), "{err}");
        assert!(parse_mc_login_response(500, b"").is_err());
    }

    #[test]
    fn entitlements() {
        let owned = br#"{"items":[{"name":"product_minecraft","signature":"x"},{"name":"game_minecraft","signature":"y"}],"signature":"s","keyId":"1"}"#;
        assert!(parse_entitlements(200, owned).unwrap());
        assert!(!parse_entitlements(200, br#"{"items":[]}"#).unwrap());
        assert!(parse_entitlements(401, b"").is_err());
    }

    #[test]
    fn profile() {
        let body = br#"{"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch",
            "skins":[{"id":"s1","state":"ACTIVE","url":"http://textures.minecraft.net/texture/abc","variant":"CLASSIC"}],
            "capes":[{"id":"c1","state":"INACTIVE","url":"http://textures.minecraft.net/texture/cape","alias":"Migrator"}]}"#;
        let p = parse_profile(200, body).unwrap();
        assert_eq!((p.id.as_str(), p.name.as_str()), ("069a79f444e94726a5befca90e38aaf5", "Notch"));
        assert_eq!(p.skins[0].variant, "CLASSIC");
        assert_eq!(p.capes[0].alias, "Migrator");
        assert_eq!(p.active_skin_url().as_deref(), Some("http://textures.minecraft.net/texture/abc"));

        let err = parse_profile(404, br#"{"path":"/minecraft/profile","errorType":"NOT_FOUND"}"#).unwrap_err().to_string();
        assert!(err.contains("doesn't own Minecraft Java Edition"), "{err}");
        assert!(parse_profile(200, br#"{"id":"a","name":"b"}"#).unwrap().skins.is_empty());
    }

    #[test]
    fn jwt_claims() {
        // header.{"xuid":"2535","n":7}.sig
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"xuid":"2535","n":7}"#);
        let jwt = format!("h.{payload}.s");
        assert_eq!(jwt_claim(&jwt, "xuid").as_deref(), Some("2535"));
        assert_eq!(jwt_claim(&jwt, "n").as_deref(), Some("7"));
        assert_eq!(jwt_claim(&jwt, "missing"), None);
        assert_eq!(jwt_claim("garbage", "xuid"), None);
    }
}
