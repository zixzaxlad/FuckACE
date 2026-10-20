import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Accordion, AccordionDetails, AccordionSummary, Box, Button, Paper, TextField, Typography } from '@mui/material';
import { ExpandMore, PlayArrow, Stop } from '@mui/icons-material';
import {
  cancelLolOrigin, captureLolClick, getRunningLolRoot, loadLaunchConfig,
  saveLaunchConfig, screenshotSteps, startLolOrigin,
} from '../services/gameLauncher';

interface LolLauncherCardProps {
  addLog: (message: string) => void;
}

export function LolLauncherCard({ addLog }: LolLauncherCardProps) {
  const [config, setConfig] = useState(loadLaunchConfig);
  const [running, setRunning] = useState(false);
  const [recording, setRecording] = useState<number | null>(null);
  const [status, setStatus] = useState('就绪 · QQ 需已登录，大区需已选为艾欧尼亚');
  const [error, setError] = useState(false);

  useEffect(() => {
    const subscription = listen<string>('lol-launch-status', ({ payload }) => {
      setStatus(payload);
      addLog(`[LOL] ${payload}`);
    });
    return () => { void subscription.then((unlisten) => unlisten()); };
  }, [addLog]);

  useEffect(() => { saveLaunchConfig(config); }, [config]);

  const reportError = (failure: unknown) => {
    setStatus(String(failure));
    setError(true);
    addLog(`[LOL] ${failure}`);
  };

  const launch = async () => {
    setRunning(true);
    setError(false);
    setStatus('正在检查启动环境…');
    try {
      const message = await startLolOrigin(config);
      setStatus(message);
      addLog(`[LOL] ${message}`);
    } catch (failure) {
      reportError(failure);
    } finally {
      setRunning(false);
    }
  };

  const detectRoot = async () => {
    try {
      const root = await getRunningLolRoot();
      setConfig({ root, steps: screenshotSteps(root) });
      setStatus(`游戏目录已读取：${root}`);
      setError(false);
    } catch (failure) { reportError(failure); }
  };

  const record = async (index: number) => {
    setRecording(index);
    setError(false);
    setStatus('5 秒后记录鼠标位置，请切到登录器并将鼠标停在对应按钮上（无需点击）');
    try {
      const step = config.steps[index];
      const captured = await captureLolClick(config.root, step.label, step.delaySeconds);
      setConfig((previous) => ({ ...previous, steps: previous.steps.map((value, i) => i === index ? captured : value) }));
      setStatus(`已录制：${step.label}`);
    } catch (failure) { reportError(failure); }
    finally { setRecording(null); }
  };

  const busy = running || recording !== null;

  return (
    <Paper elevation={2} sx={{ p: 1.5, flexShrink: 0 }}>
      <Box display="flex" gap={1} alignItems="center" flexWrap="wrap">
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mr: 'auto' }}>英雄联盟 · Origin 自动启动</Typography>
        <Button size="small" variant="contained" startIcon={<PlayArrow />} disabled={busy || !config.root} onClick={() => { void launch(); }}>一键启动</Button>
        <Button size="small" variant="outlined" startIcon={<Stop />} disabled={!running} onClick={() => { void cancelLolOrigin().catch(reportError); }}>取消引导</Button>
      </Box>
      <Typography role="status" aria-live="polite" variant="body2" color={error ? 'error' : 'text.secondary'} sx={{ mt: 0.5, overflowWrap: 'anywhere' }}>{status}</Typography>
      <Accordion disableGutters elevation={0} sx={{ mt: 0.5, '&:before': { display: 'none' } }}>
        <AccordionSummary expandIcon={<ExpandMore />} sx={{ p: 0, minHeight: 32 }}>
          <Typography variant="body2">目录与点击设置 · 已内置截图中的四步操作</Typography>
        </AccordionSummary>
        <AccordionDetails sx={{ p: 0, pt: 1 }}>
          <Box display="flex" gap={1} alignItems="center" flexWrap="wrap">
            <TextField size="small" label="LOL 根目录（包含 Game、Launcher、LeagueClient）" value={config.root} disabled={busy} sx={{ flex: '1 1 350px' }} onChange={(event) => {
              const root = event.target.value.replace(/\//g, '\\');
              setConfig({ root, steps: config.steps.map((step) => ({ ...step, executable: `${root.replace(/\\+$/, '')}\\Launcher\\Client.exe` })) });
            }} />
            <Button size="small" disabled={busy} onClick={() => { void detectRoot(); }}>读取运行中的登录器目录</Button>
          </Box>
          <Typography variant="body2" color="text.secondary" sx={{ my: 1 }}>预设来自 4K 截图：登录器客户区 1920×1152。坐标相对窗口左上角；QQ 需已登录，协议需已勾选，大区需已记住为艾欧尼亚。点击期间请勿操作鼠标。窗口大小或缩放变化后可重新录制。</Typography>
          <Box display="flex" flexDirection="column" gap={1}>
            {config.steps.map((step, index) => (
              <Box key={index} display="flex" alignItems="center" flexWrap="wrap" gap={1}>
                <Typography variant="body2" sx={{ flex: '1 1 200px' }}>{index + 1}. {step.label}</Typography>
                {(['x', 'y', 'delaySeconds'] as const).map((field) => (
                  <TextField key={field} size="small" type="number" label={field === 'delaySeconds' ? '点击前等待（秒）' : field.toUpperCase()} value={step[field]} disabled={busy}
                    sx={{ width: field === 'delaySeconds' ? 145 : 90 }} onChange={(event) => {
                      const value = Number(event.target.value);
                      setConfig((previous) => ({ ...previous, steps: previous.steps.map((item, i) => i === index ? { ...item, [field]: value } : item) }));
                    }} />
                ))}
                <Button size="small" disabled={busy || !config.root} onClick={() => { void record(index); }}>{recording === index ? '记录中…' : '5 秒后录制'}</Button>
              </Box>
            ))}
          </Box>
          <Button size="small" sx={{ mt: 1 }} disabled={busy} onClick={() => setConfig({ ...config, steps: screenshotSteps(config.root) })}>恢复截图预设</Button>
        </AccordionDetails>
      </Accordion>
    </Paper>
  );
}
