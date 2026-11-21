use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, State};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClientRect, GetCursorPos, GetForegroundWindow,
    GetWindowThreadProcessId, IsWindowVisible, SetCursorPos, SetForegroundWindow, ShowWindow,
    WindowFromPoint, GA_ROOT, SW_RESTORE,
};

#[derive(Default)]
pub struct LauncherState {
    running: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickStep {
    label: String,
    executable: PathBuf,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    delay_seconds: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    root: PathBuf,
    steps: Vec<ClickStep>,
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&right.to_string_lossy().replace('/', "\\"))
}

fn process_system() -> System {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
}

fn window_executable(window: HWND, system: &System) -> Result<PathBuf, String> {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    system
        .process(Pid::from_u32(pid))
        .and_then(|process| process.exe())
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法读取目标窗口的程序路径，请以管理员身份运行".to_string())
}

fn validate_steps(config: &LaunchConfig) -> Result<(), String> {
    if config.steps.is_empty() {
        return Err("请先录制快捷登录、选择大区和进入游戏的点击步骤".into());
    }
    for step in &config.steps {
        let relative = step
            .executable
            .strip_prefix(&config.root)
            .map_err(|_| format!("{}：录制窗口不属于当前 LOL 目录，请重新录制", step.label))?;
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || step.width <= 0
            || step.height <= 0
            || step.x < 0
            || step.y < 0
            || step.x >= step.width
            || step.y >= step.height
            || step.delay_seconds > 120
        {
            return Err(format!(
                "{}：点击位置或等待时间无效，请重新录制",
                step.label
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_running_lol_root() -> Result<PathBuf, String> {
    let system = process_system();
    for process in system.processes().values() {
        if process.name().eq_ignore_ascii_case("Client.exe") {
            if let Some(path) = process.exe() {
                if let Some(root) = path.parent().and_then(Path::parent) {
                    if root
                        .join("Game/AntiCheatExpert/SGuard/x64/SGuard64.exe")
                        .is_file()
                    {
                        return Ok(root.to_path_buf());
                    }
                }
            }
        }
    }
    Err("未找到运行中的 LOL 原生登录器，请填写游戏根目录，或打开登录器后重试".into())
}

#[tauri::command]
pub async fn capture_lol_click(
    root: PathBuf,
    label: String,
    delay_seconds: u64,
) -> Result<ClickStep, String> {
    // 留出时间让用户切到登录器，只记录鼠标位置，不执行点击。
    tokio::time::sleep(Duration::from_secs(5)).await;
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).map_err(|error| error.to_string())?;
        let window = GetAncestor(WindowFromPoint(point), GA_ROOT);
        let executable = window_executable(window, &process_system())?;
        let mut rect = RECT::default();
        GetClientRect(window, &mut rect).map_err(|error| error.to_string())?;
        ScreenToClient(window, &mut point)
            .ok()
            .map_err(|error| error.to_string())?;
        let step = ClickStep {
            label,
            executable,
            x: point.x,
            y: point.y,
            width: rect.right,
            height: rect.bottom,
            delay_seconds,
        };
        validate_steps(&LaunchConfig {
            root,
            steps: vec![step.clone()],
        })?;
        Ok(step)
    }
}

unsafe extern "system" fn collect_windows(window: HWND, parameter: LPARAM) -> BOOL {
    if IsWindowVisible(window).as_bool() {
        let windows = &mut *(parameter.0 as *mut Vec<HWND>);
        windows.push(window);
    }
    BOOL(1)
}

fn find_window(step: &ClickStep) -> Result<Option<HWND>, String> {
    let mut windows: Vec<HWND> = Vec::new();
    unsafe {
        EnumWindows(
            Some(collect_windows),
            LPARAM(&mut windows as *mut _ as isize),
        )
        .map_err(|error| error.to_string())?;
    }
    let system = process_system();
    let mut matches = Vec::new();
    for window in windows {
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut pid));
        }
        if system
            .process(Pid::from_u32(pid))
            .and_then(|process| process.exe())
            .is_some_and(|path| same_path(path, &step.executable))
        {
            let mut rect = RECT::default();
            unsafe {
                GetClientRect(window, &mut rect).map_err(|error| error.to_string())?;
            }
            if rect.right == step.width && rect.bottom == step.height {
                matches.push(window);
            }
        }
    }
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0])),
        _ => Err(format!(
            "{}：发现多个相同的窗口，请关闭多余窗口",
            step.label
        )),
    }
}

fn emit(app: &AppHandle, message: &str) -> Result<(), String> {
    app.emit("lol-launch-status", message)
        .map_err(|error| error.to_string())
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        Err("已取消引导；已启动的登录器和 ACE 保持运行".into())
    } else {
        Ok(())
    }
}

fn wait(cancelled: &AtomicBool, duration: Duration) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < duration {
        check_cancel(cancelled)?;
        std::thread::sleep(Duration::from_millis(100));
    }
    check_cancel(cancelled)
}

fn ace_ready(ace: &Path) -> Result<bool, String> {
    let system = process_system();
    let mut found = false;
    for process in system.processes().values() {
        if process.name().eq_ignore_ascii_case("SGuard64.exe") {
            let path = process
                .exe()
                .ok_or("无法确认现有 ACE 的路径，请以管理员身份运行")?;
            if !same_path(path, ace) {
                return Err(format!(
                    "其他 ACE 正在运行，请先退出对应游戏：{}",
                    path.display()
                ));
            }
            found = true;
        }
    }
    Ok(found)
}

fn start_program(path: &Path) -> Result<(), String> {
    Command::new(path)
        .current_dir(path.parent().unwrap())
        .spawn()
        .map_err(|error| format!("无法启动 {}：{}", path.display(), error))?;
    Ok(())
}

fn wait_with_ace(cancelled: &AtomicBool, duration: Duration, ace: &Path) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < duration {
        check_cancel(cancelled)?;
        if !ace_ready(ace)? {
            start_program(ace)?;
        }
        wait(cancelled, Duration::from_millis(500))?;
    }
    check_cancel(cancelled)
}

fn click(step: &ClickStep, window: HWND) -> Result<(), String> {
    unsafe {
        let _ = ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window)
            .ok()
            .map_err(|_| "无法将登录窗口切到前台，请关闭遮挡窗口后重试".to_string())?;
        std::thread::sleep(Duration::from_millis(200));
        if GetAncestor(GetForegroundWindow(), GA_ROOT) != window {
            return Err("登录窗口未获得焦点，已停止点击".into());
        }
        let mut point = POINT {
            x: step.x,
            y: step.y,
        };
        ClientToScreen(window, &mut point)
            .ok()
            .map_err(|error| error.to_string())?;
        if GetAncestor(WindowFromPoint(point), GA_ROOT) != window {
            return Err("目标按钮被其他窗口遮挡，已停止点击".into());
        }
        SetCursorPos(point.x, point.y).map_err(|error| error.to_string())?;
        let inputs = [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP].map(|flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        });
        if SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) != inputs.len() as u32 {
            return Err("Windows 未接受完整点击输入，已停止引导".into());
        }
    }
    Ok(())
}

fn run(app: &AppHandle, config: &LaunchConfig, cancelled: &AtomicBool) -> Result<String, String> {
    validate_steps(config)?;
    let launcher = config.root.join("Launcher/Client.exe");
    let client = config.root.join("LeagueClient/LeagueClient.exe");
    let ace = config
        .root
        .join("Game/AntiCheatExpert/SGuard/x64/SGuard64.exe");
    for path in [&launcher, &client, &ace] {
        if !path.is_file() {
            return Err(format!("游戏文件不存在：{}", path.display()));
        }
    }
    let system = process_system();
    if system
        .processes()
        .values()
        .any(|process| process.exe().is_some_and(|path| same_path(path, &client)))
    {
        return Err("LOL 客户端已运行，请先退出再执行完整自动引导".into());
    }
    crate::game_mode::ensure_enabled(app)?;
    emit(app, "正在启动游戏内置 ACE…")?;
    if !ace_ready(&ace)? {
        start_program(&ace)?;
    }
    let started = Instant::now();
    while !ace_ready(&ace)? {
        if started.elapsed() > Duration::from_secs(20) {
            return Err("内置 ACE 未在 20 秒内启动".into());
        }
        wait(cancelled, Duration::from_millis(500))?;
    }
    emit(app, &format!("ACE 路径已确认：{}", ace.display()))?;
    let launcher_open = system
        .processes()
        .values()
        .any(|process| process.exe().is_some_and(|path| same_path(path, &launcher)));
    if !launcher_open {
        start_program(&launcher)?;
    }
    for step in &config.steps {
        emit(app, &format!("等待窗口：{}", step.label))?;
        let started = Instant::now();
        loop {
            check_cancel(cancelled)?;
            if !ace_ready(&ace)? {
                start_program(&ace)?;
            }
            if find_window(step)?.is_some() {
                break;
            }
            if started.elapsed() > Duration::from_secs(120) {
                return Err(format!(
                    "{}：120 秒内未找到录制窗口；请检查登录状态、窗口大小和缩放",
                    step.label
                ));
            }
            wait(cancelled, Duration::from_millis(500))?;
        }
        emit(
            app,
            &format!("{}：等待 {} 秒后点击", step.label, step.delay_seconds),
        )?;
        wait_with_ace(cancelled, Duration::from_secs(step.delay_seconds), &ace)?;
        // 等待期间窗口可能关闭或重建，点击前重新定位。
        let current = find_window(step)?
            .ok_or_else(|| format!("{}：目标窗口已关闭或大小改变", step.label))?;
        check_cancel(cancelled)?;
        click(step, current)?;
        emit(app, &format!("已点击：{}", step.label))?;
    }
    emit(app, "点击完成，等待 LOL 客户端启动…")?;
    let started = Instant::now();
    loop {
        check_cancel(cancelled)?;
        if !ace_ready(&ace)? {
            start_program(&ace)?;
        }
        if process_system()
            .processes()
            .values()
            .any(|process| process.exe().is_some_and(|path| same_path(path, &client)))
        {
            return Ok("LOL 客户端进程已启动，内置 ACE 路径已确认；引导结束".into());
        }
        if started.elapsed() > Duration::from_secs(120) {
            return Err("点击完成后 120 秒内未检测到 LOL 客户端，请检查登录或大区选择".into());
        }
        wait(cancelled, Duration::from_secs(1))?;
    }
}

#[tauri::command]
pub async fn start_lol_origin(
    app: AppHandle,
    state: State<'_, LauncherState>,
    config: LaunchConfig,
) -> Result<String, String> {
    state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "已有 LOL 引导任务正在运行".to_string())?;
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = state.cancelled.clone();
    let result = tauri::async_runtime::spawn_blocking(move || run(&app, &config, &cancelled)).await;
    state.running.store(false, Ordering::SeqCst);
    result.map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn cancel_lol_origin(state: State<'_, LauncherState>) {
    state.cancelled.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_existing_lol_ace_with_windows_path_separators() {
        let running =
            Path::new(r"E:\WeGameApps\英雄联盟\Game\AntiCheatExpert\SGuard\x64\SGuard64.exe");
        let expected = Path::new(r"E:\WeGameApps\英雄联盟")
            .join("Game/AntiCheatExpert/SGuard/x64/SGuard64.exe");
        assert!(same_path(running, &expected));
        assert!(!same_path(running, Path::new(r"C:\OtherGame\SGuard64.exe")));
    }

    #[test]
    fn rejects_click_outside_game_and_invalid_coordinates() {
        let mut step = ClickStep {
            label: "登录".into(),
            executable: "D:/Other/Client.exe".into(),
            x: 20,
            y: 20,
            width: 800,
            height: 600,
            delay_seconds: 2,
        };
        let config = |step| LaunchConfig {
            root: "D:/LOL".into(),
            steps: vec![step],
        };
        assert!(validate_steps(&config(step.clone())).is_err());
        step.executable = "D:/LOL/Launcher/Client.exe".into();
        assert!(validate_steps(&config(step.clone())).is_ok());
        step.x = 800;
        assert!(validate_steps(&config(step)).is_err());
    }

    #[test]
    fn cancellation_interrupts_wait_without_clicking() {
        assert!(wait(&AtomicBool::new(true), Duration::from_secs(20)).is_err());
    }
}
