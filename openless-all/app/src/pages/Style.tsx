// Style.tsx — 接 getSettings / setDefaultPolishMode / setStyleEnabled。
// defaultMode 来自 prefs.defaultMode，启停从 prefs.enabledModes 反推。

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getSettings, setDefaultPolishMode, setStyleEnabled, setSettings } from '../lib/ipc';
import type { PolishMode, UserPreferences } from '../lib/types';
import { PageHeader, Pill } from './_atoms';

interface StyleDef {
  id: PolishMode;
  name: string;
  desc: string;
  sample: string;
}

const STYLE_IDS: PolishMode[] = ['raw', 'light', 'structured', 'formal'];

export function Style() {
  const { t } = useTranslation();
  const STYLES: StyleDef[] = STYLE_IDS.map(id => ({
    id,
    name: t(`style.modes.${id}.name`),
    desc: t(`style.modes.${id}.desc`),
    sample: t(`style.modes.${id}.sample`),
  }));
  const [prefs, setPrefs] = useState<UserPreferences | null>(null);

  useEffect(() => {
    getSettings().then(setPrefs);
  }, []);

  const onPickDefault = async (mode: PolishMode) => {
    if (!prefs) return;
    setPrefs({ ...prefs, defaultMode: mode });
    await setDefaultPolishMode(mode);
  };

  const onToggleEnabled = async (mode: PolishMode) => {
    if (!prefs) return;
    const enabled = !prefs.enabledModes.includes(mode);
    const nextEnabled = enabled
      ? [...prefs.enabledModes, mode]
      : prefs.enabledModes.filter(m => m !== mode);
    setPrefs({ ...prefs, enabledModes: nextEnabled });
    await setStyleEnabled(mode, enabled);
  };

  if (!prefs) {
    return (
      <PageHeader
        kicker={t('style.kicker')}
        title={t('style.title')}
        desc={t('common.loading')}
      />
    );
  }

  const masterEnabled = prefs.enabledModes.length > 0;

  const onMasterToggle = async () => {
    if (!prefs) return;
    if (masterEnabled) {
      // 全部关闭 → 留 raw 和当前 default 兜底
      const next = { ...prefs, enabledModes: [] as PolishMode[] };
      setPrefs(next);
      await setSettings(next);
    } else {
      const next = { ...prefs, enabledModes: ['raw', 'light', 'structured', 'formal'] as PolishMode[] };
      setPrefs(next);
      await setSettings(next);
    }
  };

  return (
    <>
      <PageHeader
        kicker={t('style.kicker')}
        title={t('style.title')}
        desc={t('style.desc')}
        right={
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ fontSize: 12, color: 'var(--ol-ink-3)' }}>{t('style.masterToggle')}</span>
            <button
              onClick={onMasterToggle}
              style={{
                position: 'relative', width: 36, height: 20, borderRadius: 999, border: 0,
                background: masterEnabled ? 'var(--ol-blue)' : 'rgba(0,0,0,0.15)',
                cursor: 'default', transition: 'background 0.16s var(--ol-motion-quick)',
              }}
            >
              <span
                style={{
                  position: 'absolute', top: 2, left: masterEnabled ? 18 : 2,
                  width: 16, height: 16, borderRadius: 999, background: '#fff',
                  boxShadow: '0 1px 2px rgba(0,0,0,.2)', transition: 'left .16s var(--ol-motion-spring)',
                }}
              />
            </button>
          </div>
        }
      />
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        {STYLES.map(s => {
          const isDefault = prefs.defaultMode === s.id;
          const isEnabled = prefs.enabledModes.includes(s.id);
          return (
            <div
              key={s.id}
              style={{
                padding: 18,
                background: 'var(--ol-surface)',
                border: '0.5px solid ' + (isDefault ? 'var(--ol-blue)' : 'var(--ol-line)'),
                borderRadius: 'var(--ol-r-lg)',
                boxShadow: isDefault ? '0 0 0 3px var(--ol-blue-ring), var(--ol-shadow-sm)' : 'var(--ol-shadow-sm)',
                opacity: isEnabled ? 1 : 0.55,
                position: 'relative',
                transition: 'border-color 0.16s var(--ol-motion-quick), box-shadow 0.18s var(--ol-motion-soft), opacity 0.18s var(--ol-motion-soft)',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                <button
                  onClick={() => onPickDefault(s.id)}
                  aria-label={t('style.ariaSetDefault')}
                  style={{
                    width: 16, height: 16, padding: 0, border: 0, borderRadius: 999,
                    background: isDefault ? 'var(--ol-blue)' : 'transparent',
                    boxShadow: isDefault ? 'none' : 'inset 0 0 0 1.5px var(--ol-line-strong)',
                    color: '#fff', cursor: 'default',
                    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                  }}
                >
                  {isDefault && (
                    <svg width="9" height="9" viewBox="0 0 9 9"><path d="M1.5 4.5l2.5 2.5 4-5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" /></svg>
                  )}
                </button>
                <button
                  onClick={() => onPickDefault(s.id)}
                  style={{
                    background: 'transparent', border: 0, padding: 0,
                    fontSize: 14, fontWeight: 600, fontFamily: 'inherit',
                    color: 'var(--ol-ink)', cursor: 'default',
                  }}
                >
                  {s.name}
                </button>
                {isDefault && <Pill tone="blue" size="sm" style={{ marginLeft: 'auto' }}>{t('style.currentDefault')}</Pill>}
                {!isDefault && (
                  <button
                    onClick={() => onToggleEnabled(s.id)}
                    style={{
                      marginLeft: 'auto',
                      position: 'relative', width: 30, height: 18, borderRadius: 999, border: 0,
                      background: isEnabled ? 'var(--ol-blue)' : 'rgba(0,0,0,0.15)',
                      cursor: 'default',
                      transition: 'background 0.16s var(--ol-motion-quick)',
                    }}
                  >
                    <span style={{
                      position: 'absolute', top: 2, left: isEnabled ? 14 : 2,
                      width: 14, height: 14, borderRadius: 999, background: '#fff',
                      boxShadow: '0 1px 2px rgba(0,0,0,.2)', transition: 'left .16s var(--ol-motion-spring)',
                    }} />
                  </button>
                )}
              </div>
              <div style={{ fontSize: 11.5, color: 'var(--ol-ink-3)', marginBottom: 12 }}>{s.desc}</div>
              <div
                style={{
                  fontSize: 12.5, color: 'var(--ol-ink-2)', lineHeight: 1.6,
                  padding: 12, borderRadius: 8,
                  background: 'var(--ol-surface-2)',
                  border: '0.5px dashed var(--ol-line)',
                  whiteSpace: 'pre-line',
                }}
              >
                {s.sample}
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}
