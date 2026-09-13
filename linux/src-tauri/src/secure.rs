use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.playrood.yood";
const ACCOUNT_HINTS: &str = "account-hints";

/// Yood never writes Google cookies, passwords, OAuth tokens, or WebView
/// session data here. The platform WebView owns its encrypted cookie store.
/// This keyring entry is only a convenience label for the UI's account list.
pub fn get_account_hints() -> AppResult<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT_HINTS)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(AppError::from(error)),
    }
}

pub fn set_account_hints(value: &str) -> AppResult<()> {
    if value.len() > 4096 || value.chars().any(|c| c.is_control()) {
        return Err(AppError::InvalidInput("invalid account label".into()));
    }
    keyring::Entry::new(SERVICE, ACCOUNT_HINTS)?.set_password(value)?;
    Ok(())
}

pub fn clear_account_hints() -> AppResult<()> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT_HINTS)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::from(error)),
    }
}
