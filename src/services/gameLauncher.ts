import { invoke } from '@tauri-apps/api/core';

export interface ClickStep {
  label: string;
  executable: string;
  x: number;
  y: number;
  width: number;
  height: number;
  delaySeconds: number;
}

export interface LaunchConfig {
  root: string;
  steps: ClickStep[];
}

// 用户提供的 3840×2160 截图，登录器客户区为 1920×1152。
export function screenshotSteps(root: string): ClickStep[] {
  const executable = `${root.replace(/[/\\]+$/, '')}\\Launcher\\Client.exe`;
  return [
    { label: 'QQ 登录', x: 1721, y: 482, delaySeconds: 8 },
    { label: '切换至快速登录', x: 1721, y: 923, delaySeconds: 3 },
    { label: '确定快捷登录', x: 1721, y: 842, delaySeconds: 3 },
    { label: '确认选择大区（艾欧尼亚）', x: 1371, y: 953, delaySeconds: 8 },
  ].map((step) => ({ ...step, executable, width: 1920, height: 1152 }));
}

const configKey = 'fuckace.lol-origin';

export function loadLaunchConfig(): LaunchConfig {
  const saved = localStorage.getItem(configKey);
  return saved === null ? { root: '', steps: screenshotSteps('') } : JSON.parse(saved);
}

export function saveLaunchConfig(config: LaunchConfig) {
  localStorage.setItem(configKey, JSON.stringify(config));
}

export function startLolOrigin(config: LaunchConfig) {
  return invoke<string>('start_lol_origin', { config });
}

export function cancelLolOrigin() {
  return invoke<void>('cancel_lol_origin');
}

export function getRunningLolRoot() {
  return invoke<string>('get_running_lol_root');
}

export function captureLolClick(root: string, label: string, delaySeconds: number) {
  return invoke<ClickStep>('capture_lol_click', { root, label, delaySeconds });
}
