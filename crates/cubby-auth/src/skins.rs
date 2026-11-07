//! Skin and cape of an account, through the Minecraft services (`api.minecraftservices.com`): read the profile
//! (the skin and every cape the account owns, with the one worn), change the skin, show or hide a cape.
//! Every call needs the account's Minecraft access token (see [`crate::refresh`]).

use cubby_common::net::Http;
use cubby_common::{Error, Result};
use serde_json::json;

use crate::xbox::{get_bearer, parse_profile, send, Profile, PROFILE_URL};

const SKINS_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";
const ACTIVE_SKIN_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins/active";
const ACTIVE_CAPE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/capes/active";

/// Largest skin Cubby sends: a skin is a 64×64 or 64×32 PNG, far under this.
pub const MAX_SKIN_BYTES: usize = 64 * 1024;

pub const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// The skin and capes of the account.
pub async fn fetch_profile(http: &Http, token: &str) -> Result<Profile> {
    let (status, body) = get_bearer(http, PROFILE_URL, token).await?;
    parse_profile(status, &body)
}

/// Makes `png` (a 64×64 or 64×32 skin) the active skin, with the arms `classic` or `slim`.
pub async fn set_skin(http: &Http, token: &str, variant: &str, png: Vec<u8>) -> Result<Profile> {
    let variant = check_variant(variant)?;
    check_png(&png)?;
    let part = reqwest::multipart::Part::bytes(png).file_name("skin.png").mime_str("image/png").map_err(|e| Error::Other(e.to_string()))?;
    let form = reqwest::multipart::Form::new().text("variant", variant.to_string()).part("file", part);
    let (status, body) = send(http.client().post(SKINS_URL).bearer_auth(token).multipart(form), SKINS_URL).await?;
    parse_change(status, &body, "change the skin")
}

/// Goes back to the default skin of the account.
pub async fn reset_skin(http: &Http, token: &str) -> Result<()> {
    let (status, body) = send(http.client().delete(ACTIVE_SKIN_URL).bearer_auth(token), ACTIVE_SKIN_URL).await?;
    parse_change(status, &body, "reset the skin").map(|_| ())
}

/// Shows the cape with this id, or hides the cape when `cape_id` is `None`.
pub async fn set_active_cape(http: &Http, token: &str, cape_id: Option<&str>) -> Result<()> {
    let req = match cape_id {
        Some(id) => {
            check_cape_id(id)?;
            http.client().put(ACTIVE_CAPE_URL).bearer_auth(token).json(&json!({ "capeId": id }))
        }
        None => http.client().delete(ACTIVE_CAPE_URL).bearer_auth(token),
    };
    let (status, body) = send(req, ACTIVE_CAPE_URL).await?;
    parse_change(status, &body, "change the cape").map(|_| ())
}

fn check_variant(variant: &str) -> Result<&'static str> {
    match variant.to_ascii_lowercase().as_str() {
        "classic" => Ok("classic"),
        "slim" => Ok("slim"),
        other => Err(Error::Other(format!("Unknown arm model: {other}."))),
    }
}

fn check_cape_id(id: &str) -> Result<()> {
    // cape ids are UUIDs
    if !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        Ok(())
    } else {
        Err(Error::Other("That is not a cape id.".into()))
    }
}

/// A skin must be a PNG, small, and 64×64 or 64×32 (the width and height are in the header).
pub fn check_png(png: &[u8]) -> Result<()> {
    if png.len() > MAX_SKIN_BYTES || png.len() < 33 || png[..8] != PNG_MAGIC {
        return Err(Error::Other("A skin is a 64×64 or 64×32 PNG.".into()));
    }
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    if width != 64 || (height != 64 && height != 32) {
        return Err(Error::Other("A skin is a 64×64 or 64×32 PNG.".into()));
    }
    Ok(())
}

/// Turns the answer to a change into the new profile (or what went wrong, in words).
fn parse_change(status: u16, body: &[u8], what: &str) -> Result<Profile> {
    match status {
        200..=299 => Ok(serde_json::from_slice(body).unwrap_or(Profile { id: String::new(), name: String::new(), skins: Vec::new(), capes: Vec::new() })),
        401 | 403 => Err(Error::Auth(format!("Mojang refused to {what}. Sign in again in Accounts."))),
        429 => Err(Error::Auth(format!("Too many changes: Mojang asks to wait a moment before you {what} again."))),
        s => Err(Error::Auth(format!("Couldn't {what} (HTTP {s}). Try again later."))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_header(w: u32, h: u32) -> Vec<u8> {
        let mut v = PNG_MAGIC.to_vec();
        v.extend_from_slice(&[0, 0, 0, 13]);
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        v
    }

    #[test]
    fn only_skin_sized_pngs_pass() {
        assert!(check_png(&png_header(64, 64)).is_ok());
        assert!(check_png(&png_header(64, 32)).is_ok());
        assert!(check_png(&png_header(128, 128)).is_err());
        assert!(check_png(&png_header(64, 48)).is_err());
        assert!(check_png(b"GIF89a not a png at all, but long enough to be read as a header").is_err());
        assert!(check_png(&[]).is_err());
        let mut big = png_header(64, 64);
        big.resize(MAX_SKIN_BYTES + 1, 0);
        assert!(check_png(&big).is_err());
    }

    #[test]
    fn variants_and_cape_ids() {
        assert_eq!(check_variant("SLIM").unwrap(), "slim");
        assert_eq!(check_variant("classic").unwrap(), "classic");
        assert!(check_variant("wide").is_err());
        assert!(check_cape_id("a4fd3e24-b9ae-4f43-bca1-3b9b0b9d7e0a").is_ok());
        assert!(check_cape_id("").is_err());
        assert!(check_cape_id("../x").is_err());
    }

    #[test]
    fn changes_report_what_went_wrong() {
        assert!(parse_change(200, br#"{"id":"abc","name":"N","skins":[],"capes":[]}"#, "change the skin").is_ok());
        assert!(parse_change(204, b"", "change the cape").is_ok());
        let e = parse_change(401, b"", "change the skin").unwrap_err().to_string();
        assert!(e.contains("Sign in again"), "{e}");
        assert!(parse_change(429, b"", "change the skin").unwrap_err().to_string().contains("wait"));
        assert!(parse_change(500, b"", "change the skin").unwrap_err().to_string().contains("500"));
    }
}
