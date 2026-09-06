use crate::api::Result;
use theseus::prelude::*;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("connectivity")
        .invoke_handler(tauri::generate_handler![
            check,
            get_offline,
            set_offline,
        ])
        .build()
}

/// Probe network reachability, update `State.offline`, and return the new
/// offline flag.
#[tauri::command]
pub async fn check() -> Result<bool> {
    let state = State::get().await?;
    let reachable = minecraft_auth::check_reachable().await.is_ok();
    state.set_offline(!reachable);
    Ok(state.is_offline())
}

#[tauri::command]
pub async fn get_offline() -> Result<bool> {
    let state = State::get().await?;
    Ok(state.is_offline())
}

#[tauri::command]
pub async fn set_offline(offline: bool) -> Result<()> {
    let state = State::get().await?;
    state.set_offline(offline);
    Ok(())
}