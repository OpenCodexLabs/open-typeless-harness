// SettingsModal.tsx — centered sheet with sub-nav on the left.
//
// 设计原则：每个可见控件都必须可用。没有后端支撑的占位（账号 / 主题切换 / 启动项 /
// 开机自启）已从此弹窗移除，避免 "看似可点实际无效" 的负面体感。
// 待 backend 就位后再补回（参见 issue #69）。

import { useEffect, useState, type CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { Icon } from './Icon';
import { AboutUpdateControl, Settings as SettingsContent, type SettingsSectionId } from '../pages/Settings';
import { Row } from './ui/Row';
import { readFontScale, setFontScale, type FontScaleId } from '../lib/fontScale';
import { openExternal } from '../lib/ipc';
import {
  FOLLOW_SYSTEM,
  getLocalePreference,
  setLocalePreference,
  type SupportedLocale,
} from '../i18n';
import type { OS } from './WindowChrome';

interface SettingsModalProps {
  os: OS;
  onClose: () => void;
  initialSettingsSection?: SettingsSectionId;
}

// 稳定 ID（与 i18n key 一致，方便 modal.sections.* 渲染）。
type ModalSectionId = 'settings' | 'personalize' | 'about';

interface ModalNavItem {
  id: string;
  icon: string;
  external?: boolean;
  href?: string;
}

interface ModalGroup {
  items: ModalNavItem[];
}

const REPO_URL = 'https://github.com/OpenClaudex/open-typeless-harness';
const HELP_URL = `${REPO_URL}#readme`;
const RELEASE_NOTES_URL = `${REPO_URL}/releases`;

export function SettingsModal({ os: _os, onClose, initialSettingsSection }: SettingsModalProps) {
  const { t } = useTranslation();
  const [section, setSection] = useState<ModalSectionId>('settings');
  const groups: ModalGroup[] = [
    {
      items: [
        { id: 'settings', icon: 'settings' },
        { id: 'personalize', icon: 'sparkle' },
        { id: 'about', icon: 'info' },
      ],
    },
    {
      items: [
        { id: 'helpCenter', icon: 'help', external: true, href: HELP_URL },
        { id: 'releaseNotes', icon: 'doc', external: true, href: RELEASE_NOTES_URL },
      ],
    },
  ];

  return (
    <div
      onClick={onClose}
      style={{
        position: 'absolute', inset: 0,
        background: 'rgba(15,17,22,0.32)',
        backdropFilter: 'blur(8px) saturate(140%)',
        WebkitBackdropFilter: 'blur(8px) saturate(140%)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        padding: 28,
        zIndex: 50,
        animation: 'ol-modal-fade .2s var(--ol-motion-soft)',
      }}>

      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: '100%', maxWidth: 880, height: '100%', maxHeight: 600,
          background: 'var(--ol-surface)',
          borderRadius: 14,
          border: '0.5px solid rgba(0,0,0,.08)',
          boxShadow: '0 30px 80px -20px rgba(15,17,22,.35), 0 0 0 0.5px rgba(0,0,0,.06)',
          display: 'flex', overflow: 'hidden',
          animation: 'ol-modal-pop .28s var(--ol-motion-spring)',
          position: 'relative',
        }}>

        {/* sub-sidebar */}
        <aside
          style={{
            width: 200, flexShrink: 0,
            background: 'rgba(247,247,250,0.7)',
            borderRight: '0.5px solid var(--ol-line-soft)',
            padding: '18px 12px',
            display: 'flex', flexDirection: 'column', gap: 14,
          }}>

          {groups.map((g, gi) => (
            <div key={gi} style={{ display: 'flex', flexDirection: 'column', gap: 1, paddingTop: gi === 1 ? 8 : 0, borderTop: gi === 1 ? '0.5px solid var(--ol-line-soft)' : 'none' }}>
              {g.items.map((it) => {
                const active = section === it.id && !it.external;
                return (
                  <button
                    key={it.id}
                    onClick={() => {
                      if (it.external && it.href) {
                        void openExternal(it.href);
                      } else {
                        setSection(it.id as ModalSectionId);
                      }
                    }}
                    style={{
                      display: 'flex', alignItems: 'center', gap: 10,
                      padding: '7px 10px',
                      borderRadius: 8, border: 0,
                      background: active ? '#fff' : 'transparent',
                      color: active ? 'var(--ol-ink)' : 'var(--ol-ink-3)',
                      fontFamily: 'inherit', fontSize: 13, fontWeight: active ? 600 : 500,
                      boxShadow: active ? '0 1px 2px rgba(0,0,0,.05), 0 0 0 0.5px rgba(0,0,0,.06)' : 'none',
                      cursor: 'default', textAlign: 'left',
                      transition: 'background 0.16s var(--ol-motion-quick), color 0.16s var(--ol-motion-quick)',
                    }}>

                    <Icon name={it.icon} size={14} />
                    <span style={{ flex: 1 }}>{t(`modal.sections.${it.id}`)}</span>
                    {it.external && <Icon name="external" size={11} />}
                  </button>
                );
              })}
            </div>
          ))}
        </aside>

        {/* content — 父容器 overflow:hidden + 列向 flex；X 和 h2 固定在头部，
            只有最里层的 scroll wrapper 真正滚动。这样模态左 sidebar、关闭按钮、
            section 标题都不会跟着内容一起飘。 */}
        <div style={{ flex: 1, minWidth: 0, overflow: 'hidden', position: 'relative', display: 'flex', flexDirection: 'column' }}>
          <button
            onClick={onClose}
            style={{
              position: 'absolute', top: 14, right: 14, zIndex: 2,
              width: 28, height: 28, border: 0, borderRadius: 999,
              background: 'transparent', color: 'var(--ol-ink-3)',
              display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
              cursor: 'default',
              transition: 'background 0.16s var(--ol-motion-quick)',
            }}
            onMouseEnter={e => (e.currentTarget.style.background = 'rgba(0,0,0,0.05)')}
            onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
            title={t('common.close')}>

            <Icon name="close" size={14} />
          </button>

          <h2 style={{ margin: 0, padding: '22px 28px 8px', fontSize: 22, fontWeight: 600, letterSpacing: '-0.02em', flexShrink: 0 }}>{t(`modal.sections.${section}`)}</h2>

          {section === 'settings' ? (
            // SettingsContent 自己接管 flex:1 + 内部右栏 scroll，外层不能再加 overflow:auto。
            <div style={{ flex: 1, minHeight: 0, padding: '10px 28px 28px', display: 'flex', flexDirection: 'column' }}>
              <SettingsContent embedded initialSection={initialSettingsSection} />
            </div>
          ) : (
            // personalize / about 短内容：单一 scroll wrapper，超出时本块滚动。
            <div className="ol-thinscroll" style={{ flex: 1, minHeight: 0, overflow: 'auto', padding: '10px 28px 28px' }}>
              {section === 'personalize' && <PersonalizeSection />}
              {section === 'about' && <AboutMini />}
            </div>
          )}
        </div>
      </div>

      <style>{`
        @keyframes ol-modal-fade {
          from { opacity: 0; backdrop-filter: blur(0); -webkit-backdrop-filter: blur(0); }
          to   { opacity: 1; backdrop-filter: blur(8px) saturate(140%); -webkit-backdrop-filter: blur(8px) saturate(140%); }
        }
        @keyframes ol-modal-pop {
          from { opacity: 0; transform: translateY(8px) scale(.98); filter: blur(8px); }
          to   { opacity: 1; transform: translateY(0) scale(1); filter: blur(0); }
        }
      `}</style>
    </div>
  );
}

function PersonalizeSection() {
  const { t } = useTranslation();
  // 玻璃强度持久化到 localStorage，并实时写入 CSS var --ol-glass-blur。
  // 这是 CSS-only 的层（影响 backdrop-filter 的内层强度）；macOS NSVisualEffectView
  // 是另一层，由 Tauri 在窗口创建时一次性配置，运行时改动需要重启 App。
  const [blur, setBlur] = useState<number>(() => {
    const saved = window.localStorage.getItem('ol.glassBlur');
    return saved ? Number(saved) : 22;
  });

  useEffect(() => {
    document.documentElement.style.setProperty('--ol-glass-blur', `${blur}px`);
    window.localStorage.setItem('ol.glassBlur', String(blur));
  }, [blur]);

  const [fontScale, setFontScaleState] = useState<FontScaleId>(() => readFontScale());
  const applyFontScaleChoice = (next: FontScaleId) => {
    setFontScaleState(next);
    setFontScale(next);
  };
  const fontOptions: Array<[FontScaleId, string]> = [
    ['small', t('modal.personalize.fontSmall')],
    ['medium', t('modal.personalize.fontMedium')],
    ['large', t('modal.personalize.fontLarge')],
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <Row label={t('modal.personalize.language')}>
        <LanguagePicker />
      </Row>
      <Row label={t('modal.personalize.font')} desc={t('modal.personalize.fontDesc')}>
        <div style={{ display: 'flex', gap: 4, padding: 2, background: 'rgba(0,0,0,0.04)', borderRadius: 8 }}>
          {fontOptions.map(([id, label]) => {
            const selected = fontScale === id;
            return (
              <button
                key={id}
                onClick={() => applyFontScaleChoice(id)}
                style={{
                  minWidth: 64,
                  height: 28,
                  border: 0,
                  borderRadius: 6,
                  background: selected ? '#fff' : 'transparent',
                  color: selected ? 'var(--ol-ink)' : 'var(--ol-ink-3)',
                  fontFamily: 'inherit',
                  fontSize: 12,
                  fontWeight: selected ? 600 : 500,
                  cursor: 'default',
                  boxShadow: selected ? '0 1px 2px rgba(0,0,0,.06), 0 0 0 0.5px rgba(0,0,0,.06)' : 'none',
                  transition: 'background 0.16s var(--ol-motion-quick), color 0.16s var(--ol-motion-quick), box-shadow 0.18s var(--ol-motion-soft)',
                  padding: '0 12px',
                }}
              >
                {label}
              </button>
            );
          })}
        </div>
      </Row>
      <Row label={t('modal.personalize.blur')} desc={t('modal.personalize.blurDesc')}>
        <div style={{ display: 'inline-flex', alignItems: 'center', gap: 10 }}>
          <input
            type="range"
            min="0"
            max="48"
            value={blur}
            onChange={e => setBlur(Number(e.target.value))}
            style={{ width: 200, accentColor: 'var(--ol-blue)' }}
          />
          <span style={{ fontSize: 12, fontFamily: 'var(--ol-font-mono)', color: 'var(--ol-ink-3)', minWidth: 36 }}>
            {blur}px
          </span>
        </div>
      </Row>
    </div>
  );
}

function AboutMini() {
  const { t } = useTranslation();
  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 16 }}>
        <img src="AppIcon.png" alt="" style={{ width: 56, height: 56, borderRadius: 13, boxShadow: '0 4px 10px rgba(0,0,0,.10), 0 0 0 0.5px rgba(0,0,0,.06)' }} />
        <div>
          <div style={{ fontSize: 17, fontWeight: 600 }}>{t('app.name')}</div>
          <AboutUpdateControl tagline={t('modal.about.tagline')} />
        </div>
      </div>
      <Row label={t('modal.about.docs')}>
        <button
          style={btnGhost}
          onClick={() => openExternal(HELP_URL)}
        >
          {t('modal.about.docsBtn')}
        </button>
      </Row>
      <Row label={t('modal.about.feedback')}>
        <button
          style={btnGhost}
          onClick={() => openExternal(`${REPO_URL}/issues`)}
        >
          {t('modal.about.feedbackBtn')}
        </button>
      </Row>
      <Row label={t('modal.about.privacy')} desc={t('modal.about.privacyDesc')}>
        <span style={{ fontSize: 11, padding: '3px 8px', borderRadius: 999, background: 'var(--ol-blue-soft)', color: 'var(--ol-blue)', fontWeight: 500 }}>{t('modal.about.localFirst')}</span>
      </Row>
    </div>
  );
}

const btnGhost: CSSProperties = {
  padding: '5px 10px', fontSize: 12, borderRadius: 6,
  border: '0.5px solid var(--ol-line-strong)',
  background: '#fff', color: 'var(--ol-ink-2)',
  cursor: 'default', fontFamily: 'inherit',
  transition: 'background 0.16s var(--ol-motion-quick), border-color 0.16s var(--ol-motion-quick)',
};

// 真正可用的语言切换器 —— 用原生 <select>，与 Settings → Language 分区共享同一份 localStorage 偏好。
function LanguagePicker() {
  const { t } = useTranslation();
  const [pref, setPref] = useState<SupportedLocale | typeof FOLLOW_SYSTEM>(getLocalePreference());

  const apply = async (next: SupportedLocale | typeof FOLLOW_SYSTEM) => {
    setPref(next);
    await setLocalePreference(next);
  };

  return (
    <select
      value={pref}
      onChange={e => apply(e.target.value as SupportedLocale | typeof FOLLOW_SYSTEM)}
      style={{
        height: 32, padding: '0 10px',
        border: '0.5px solid var(--ol-line-strong)',
        borderRadius: 8, fontSize: 12.5,
        fontFamily: 'inherit', outline: 'none',
        background: 'var(--ol-surface-2)',
        minWidth: 200, cursor: 'default',
      }}
    >
      <option value={FOLLOW_SYSTEM}>{t('settings.language.followSystem')}</option>
      <option value="zh-CN">{t('settings.language.zh')}</option>
      <option value="en">{t('settings.language.en')}</option>
    </select>
  );
}
