use std::io::ErrorKind;
use tauri::{AppHandle, Emitter};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const GAME_BAR_KEY: &str = r"Software\Microsoft\GameBar";
const GAME_MODE_VALUE: &str = "AutoGameModeEnabled";

#[tauri::command]
pub fn get_game_mode() -> Result<bool, String> {
    let value = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(GAME_BAR_KEY)
        .and_then(|key| key.get_value::<u32, _>(GAME_MODE_VALUE));

    match value {
        Ok(0) => Ok(false),
        Ok(1) => Ok(true),
        Ok(value) => Err(format!("Windows 游戏模式设置值无效：{value}")),
        // Windows 默认开启游戏模式，用户未设置时可能没有此值。
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(format!("无法读取 Windows 游戏模式：{error}")),
    }
}

#[tauri::command]
pub fn set_game_mode(enabled: bool) -> Result<bool, String> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(GAME_BAR_KEY)
        .map_err(|error| format!("无法打开 Windows 游戏模式设置：{error}"))?;
    let value = u32::from(enabled);
    key.set_value(GAME_MODE_VALUE, &value)
        .map_err(|error| format!("无法设置 Windows 游戏模式：{error}"))?;
    let current = get_game_mode()?;
    if current != enabled {
        return Err("Windows 游戏模式设置未生效".into());
    }
    Ok(current)
}

pub fn ensure_enabled(app: &AppHandle) -> Result<(), String> {
    if !get_game_mode()? {
        set_game_mode(true)?;
    }
    app.emit("game-mode-changed", true)
        .map_err(|error| error.to_string())
}
