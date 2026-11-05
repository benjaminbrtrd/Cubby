//! Microsoft accounts.

use super::*;

// Accounts

pub(super) fn account_list(store: &AccountStore) -> Vec<AccountDto> {
    let default = store.default_account().map(|a| a.id.clone());
    store
        .list()
        .iter()
        .map(|a| AccountDto::new(a, default.as_deref() == Some(a.id.as_str())))
        .collect()
}

#[tauri::command]
pub async fn list_accounts(core: State<'_, Core>) -> CmdResult<Vec<AccountDto>> {
    Ok(account_list(&*core.accounts.lock().await))
}

#[tauri::command]
pub async fn start_microsoft_login(core: State<'_, Core>) -> CmdResult<DeviceCodeDto> {
    let client_id = core.msa_client_id().ok_or(NO_MSA_CLIENT_ID)?;
    let code = cubby_auth::start_device_login(&core.http, &client_id)
        .await
        .msg()?;
    let dto = DeviceCodeDto::from(&code);
    let (cancel, _) = watch::channel(false);
    if let Some(previous) = core
        .login
        .lock()
        .expect("login lock")
        .replace(PendingLogin { code, cancel })
    {
        let _ = previous.cancel.send(true);
    }
    Ok(dto)
}

#[tauri::command]
pub async fn finish_microsoft_login(core: State<'_, Core>) -> CmdResult<AccountDto> {
    let (code, cancel) = {
        let login = core.login.lock().expect("login lock");
        let pending = login.as_ref().ok_or("There's no sign-in in progress.")?;
        (pending.code.clone(), pending.cancel.subscribe())
    };
    let client_id = core.msa_client_id().ok_or(NO_MSA_CLIENT_ID)?;
    let result = cubby_auth::finish_device_login(&core.http, &client_id, &code, cancel).await;
    {
        let mut login = core.login.lock().expect("login lock");
        if login
            .as_ref()
            .is_some_and(|p| p.code.device_code == code.device_code)
        {
            *login = None;
        }
    }
    let account = result.msg()?;
    let mut accounts = core.accounts.lock().await;
    accounts.add_or_update(account.clone());
    if accounts.default_account().is_none() {
        accounts.set_default(&account.id).msg()?;
    }
    accounts.save().msg()?;
    let is_default = accounts
        .default_account()
        .is_some_and(|a| a.id == account.id);
    Ok(AccountDto::new(&account, is_default))
}

#[tauri::command]
pub fn cancel_microsoft_login(core: State<'_, Core>) {
    if let Some(pending) = core.login.lock().expect("login lock").take() {
        let _ = pending.cancel.send(true);
    }
}

#[tauri::command]
pub async fn remove_account(core: State<'_, Core>, id: String) -> CmdResult<()> {
    let mut accounts = core.accounts.lock().await;
    accounts.remove(&id);
    accounts.save().msg()
}

#[tauri::command]
pub async fn set_default_account(core: State<'_, Core>, id: String) -> CmdResult<()> {
    let mut accounts = core.accounts.lock().await;
    accounts.set_default(&id).msg()?;
    accounts.save().msg()
}
