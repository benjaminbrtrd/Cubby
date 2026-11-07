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

// Skin and cape

/// The Minecraft token of the account, renewed when it is about to expire (and saved).
async fn minecraft_token(core: &Core, id: &str) -> CmdResult<String> {
    let mut accounts = core.accounts.lock().await;
    let mut account = accounts.get(id).cloned().ok_or("That account is gone.")?;
    if account.needs_refresh() {
        let client_id = core
            .msa_client_id()
            .ok_or("Your Microsoft session expired, and no Azure application ID is set to renew it. Add one in Settings › Accounts.")?;
        cubby_auth::refresh(&core.http, &client_id, &mut account).await.msg()?;
        accounts.add_or_update(account.clone());
        accounts.save().msg()?;
    }
    let cubby_auth::AccountKind::Microsoft { mc_access_token, .. } = &account.kind;
    Ok(mc_access_token.clone())
}

/// Remembers the active skin of the account, so its face is up to date in the title bar and in Accounts.
async fn remember_skin(core: &Core, id: &str, url: Option<String>) {
    let mut accounts = core.accounts.lock().await;
    if let Some(mut account) = accounts.get(id).cloned() {
        let cubby_auth::AccountKind::Microsoft { skin_url, .. } = &mut account.kind;
        *skin_url = url;
        accounts.add_or_update(account);
        if let Err(e) = accounts.save() {
            log::warn!("could not save the skin of {id}: {e}");
        }
    }
}

/// The skin an account wears and the capes it owns, from the Minecraft services.
#[tauri::command]
pub async fn skin_profile(core: State<'_, Core>, account_id: String) -> CmdResult<SkinProfileDto> {
    let token = minecraft_token(&core, &account_id).await?;
    let profile = cubby_auth::skins::fetch_profile(&core.http, &token).await.msg()?;
    let dto = SkinProfileDto::from(profile);
    remember_skin(&core, &account_id, dto.skin_url.clone()).await;
    Ok(dto)
}

/// Shows the cape with this id, or hides the cape (`cape_id` empty).
#[tauri::command]
pub async fn set_active_cape(
    core: State<'_, Core>,
    account_id: String,
    cape_id: Option<String>,
) -> CmdResult<SkinProfileDto> {
    let token = minecraft_token(&core, &account_id).await?;
    cubby_auth::skins::set_active_cape(&core.http, &token, cape_id.as_deref())
        .await
        .msg()?;
    let profile = cubby_auth::skins::fetch_profile(&core.http, &token).await.msg()?;
    Ok(SkinProfileDto::from(profile))
}

/// Makes the PNG the active skin, with `variant` "classic" or "slim" arms.
#[tauri::command]
pub async fn set_skin(
    core: State<'_, Core>,
    account_id: String,
    variant: String,
    png: Vec<u8>,
) -> CmdResult<SkinProfileDto> {
    let token = minecraft_token(&core, &account_id).await?;
    cubby_auth::skins::set_skin(&core.http, &token, &variant, png).await.msg()?;
    let profile = cubby_auth::skins::fetch_profile(&core.http, &token).await.msg()?;
    let dto = SkinProfileDto::from(profile);
    remember_skin(&core, &account_id, dto.skin_url.clone()).await;
    Ok(dto)
}

/// The default skins of the game, read from a client jar Cubby downloaded.
#[tauri::command]
pub async fn default_skins(core: State<'_, Core>) -> CmdResult<Vec<DefaultSkinDto>> {
    let versions = core.paths.versions();
    blocking(move || {
        cubby_instance::skins::default_skins(&versions)
            .map(|skins| skins.into_iter().map(Into::into).collect())
            .map_err(dto::message)
    })
    .await
}
