import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { FormControlLabel, Switch, Tooltip, Typography } from '@mui/material';

export function GameModeSwitch() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    const refresh = async () => {
      try {
        setEnabled(await invoke<boolean>('get_game_mode'));
        setError('');
      } catch (failure) {
        setEnabled(null);
        setError(String(failure));
      }
    };

    const subscription = listen<boolean>('game-mode-changed', ({ payload }) => {
      setEnabled(payload);
      setError('');
    });
    void refresh();
    window.addEventListener('focus', refresh);
    return () => {
      window.removeEventListener('focus', refresh);
      void subscription.then((unlisten) => unlisten());
    };
  }, []);

  const change = async (checked: boolean) => {
    setSaving(true);
    try {
      setEnabled(await invoke<boolean>('set_game_mode', { enabled: checked }));
      setError('');
    } catch (failure) {
      setError(String(failure));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Tooltip title={error || '修改 Windows 设置中的游戏模式。一键启动 LOL 时会自动开启。'} placement="bottom" arrow>
      <FormControlLabel
        control={
          <Switch
            checked={enabled === true}
            onChange={(event) => { void change(event.target.checked); }}
            disabled={enabled === null || saving}
            color="success"
            size="small"
          />
        }
        label={<Typography variant="caption" color={error ? 'error' : undefined}>Windows 游戏模式</Typography>}
        sx={{ m: 0 }}
      />
    </Tooltip>
  );
}
