//! The Anthropic API key lives in the OS credential store (Windows
//! Credential Manager), never in the database or logs.

const SERVICE: &str = "Unpack";
const ACCOUNT: &str = "anthropic_api_key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("Credential store unavailable: {e}"))
}

pub fn get_api_key() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Could not read the API key: {e}")),
    }
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("The API key is empty.".to_string());
    }
    entry()?.set_password(key).map_err(|e| format!("Could not save the API key: {e}"))
}

pub fn clear_api_key() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Could not remove the API key: {e}")),
    }
}

/// Shows only the last four characters, for the Settings screen.
pub fn masked(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("\u{2026}{tail}")
}

#[cfg(test)]
mod tests {
    use super::masked;

    #[test]
    fn masked_shows_only_last_four() {
        assert_eq!(masked("sk-ant-api03-abcdefgh1234"), "\u{2026}1234");
        assert_eq!(masked("abc"), "\u{2026}abc");
    }
}
