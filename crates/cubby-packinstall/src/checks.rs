//! Small validations shared by the commands.

/// True for Minecraft 1.5.2 and older, which Cubby can't launch (Technic packs of that age are jar mods).
/// Anything that doesn't look like `1.x[.y]` is not legacy.
pub fn is_legacy_minecraft(version: &str) -> bool {
    let parts: Vec<u32> = version.trim().split('.').map_while(|p| p.parse().ok()).collect();
    if parts.len() < 2 || parts[0] != 1 {
        return false;
    }
    (parts[0], parts[1], parts.get(2).copied().unwrap_or(0)) <= (1, 5, 2)
}

/// Accepts `http://` and `https://` addresses with a host and no whitespace; the error is for the player.
pub fn check_import_url(url: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("Only http:// and https:// addresses can be imported.")?;
    if rest.split('/').next().is_none_or(|host| host.is_empty()) || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("That isn't a valid web address.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_minecraft_versions() {
        for v in ["1.5.2", "1.5", "1.4.7", "1.2.5", "1.0", "1.5.1"] {
            assert!(is_legacy_minecraft(v), "{v}");
        }
        for v in ["1.6.4", "1.7.10", "1.12.2", "1.20.1", "1.21", "", "latest", "b1.7.3", "2.0"] {
            assert!(!is_legacy_minecraft(v), "{v}");
        }
    }

    #[test]
    fn import_urls() {
        assert!(check_import_url("https://example.com/pack.mrpack").is_ok());
        assert!(check_import_url("http://example.com/pack.zip?x=1").is_ok());
        for bad in ["ftp://example.com/p.zip", "file:///C:/p.zip", "example.com/p.zip", "https://", "https:///x", "https://a b/c", "javascript:alert(1)"] {
            assert!(check_import_url(bad).is_err(), "{bad}");
        }
    }
}
