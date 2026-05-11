// Vocab.tsx — 接 Tauri 后端 list_vocab / add_vocab / remove_vocab / set_vocab_enabled。
// 数据落地到 ~/Library/Application Support/OpenLess/dictionary.json（与 Swift 同名）。

import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { addVocab, isTauri, listVocab, removeVocab, setVocabEnabled } from '../lib/ipc';
import type { DictionaryEntry, VocabPreset } from '../lib/types';
import { DEFAULT_VOCAB_PRESETS, loadVocabPresets, persistVocabPresets } from '../lib/vocabPresets';
import { Btn, Card, PageHeader } from './_atoms';

const NEW_PRESET_DRAFT_ID = '__new__';

export function Vocab() {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  const [error, setError] = useState<string | null>(null);
  const [presets, setPresets] = useState<VocabPreset[]>(DEFAULT_VOCAB_PRESETS);
  const [selectedPresetIds, setSelectedPresetIds] = useState<string[]>([]);
  const [editingPresetId, setEditingPresetId] = useState<string | null>(null);
  const [presetNameDraft, setPresetNameDraft] = useState('');
  const [presetPhrasesDraft, setPresetPhrasesDraft] = useState('');

  const refresh = async () => {
    try {
      setError(null);
      const data = await listVocab();
      setEntries(data);
    } catch (e) {
      // 之前没 try/catch,后端 decode 失败时 spinner 永久卡死。
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    void loadVocabPresets()
      .then(setPresets)
      .catch(err => setError(err instanceof Error ? err.message : String(err)));
    // 订阅后端 vocab:updated：每段口述结束、record_hits 触发后由 coordinator 推送。
    // Vocab 页面打开期间能即时看到命中数累加，无需切到其他 tab 再切回。
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const handle = await listen('vocab:updated', () => {
        void refresh();
      });
      if (cancelled) handle();
      else unlisten = handle;
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const onAdd = async () => {
    const phrase = inputRef.current?.value.trim();
    if (!phrase) return;
    await addVocab(phrase);
    if (inputRef.current) inputRef.current.value = '';
    refresh();
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      void onAdd();
    }
  };

  const onRemove = async (id: string) => {
    await removeVocab(id);
    setEntries(prev => prev.filter(e => e.id !== id));
  };

  const onToggle = async (entry: DictionaryEntry) => {
    const next = !entry.enabled;
    // 乐观更新 UI；后端失败时回滚 + 让用户看到错误，避免 UI 显示「已禁用」但 ASR/polish
    // 仍在注入此词条造成的诡异状态。issue #60。
    setEntries(prev => prev.map(e => (e.id === entry.id ? { ...e, enabled: next } : e)));
    try {
      await setVocabEnabled(entry.id, next);
    } catch (err) {
      setEntries(prev => prev.map(e => (e.id === entry.id ? { ...e, enabled: entry.enabled } : e)));
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const togglePreset = (id: string) => {
    setSelectedPresetIds(prev => (prev.includes(id) ? prev.filter(x => x !== id) : [...prev, id]));
  };

  const startEditPreset = (preset: VocabPreset) => {
    setEditingPresetId(preset.id);
    setPresetNameDraft(preset.name);
    setPresetPhrasesDraft(preset.phrases.join(', '));
  };

  const savePreset = async () => {
    if (!editingPresetId) return;
    const name = presetNameDraft.trim();
    if (!name) return;
    const phrases = Array.from(
      new Set(
        presetPhrasesDraft
          .split(/[,\n]/)
          .map(s => s.trim())
          .filter(Boolean),
      ),
    );
    const next =
      editingPresetId === NEW_PRESET_DRAFT_ID
        ? [...presets, { id: `user-${Date.now()}`, name, phrases }]
        : presets.map(p => (p.id === editingPresetId ? { ...p, name, phrases } : p));
    try {
      await persistVocabPresets(next);
      setPresets(next);
      setEditingPresetId(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const createPreset = () => {
    setEditingPresetId(NEW_PRESET_DRAFT_ID);
    setPresetNameDraft(t('vocab.presets.newPreset'));
    setPresetPhrasesDraft('');
  };

  const applySelectedPresets = async () => {
    const selected = presets.filter(p => selectedPresetIds.includes(p.id));
    if (selected.length === 0) return;
    const byPhrase = new Map<string, DictionaryEntry[]>();
    const addedPhrases = new Set<string>();
    for (const entry of entries) {
      const key = entry.phrase.trim().toLowerCase();
      if (!byPhrase.has(key)) byPhrase.set(key, []);
      byPhrase.get(key)?.push(entry);
    }
    let failures = 0;
    for (const p of selected) {
      for (const phrase of p.phrases) {
        const key = phrase.trim().toLowerCase();
        if (addedPhrases.has(key)) continue;
        const existing = byPhrase.get(key) || [];
        if (existing.length === 0) {
          try {
            await addVocab(phrase);
            addedPhrases.add(key);
          } catch {
            failures += 1;
          }
          continue;
        }
        for (const item of existing) {
          if (!item.enabled) {
            try {
              await setVocabEnabled(item.id, true);
            } catch {
              failures += 1;
            }
          }
        }
      }
    }
    await refresh();
    if (failures > 0) {
      setError(`部分词条添加失败（${failures}）`);
    }
  };

  return (
    <>
      <PageHeader
        kicker={t('vocab.kicker')}
        title={t('vocab.title')}
        desc={t('vocab.desc')}
        right={
          <div style={{ display: 'flex', gap: 8 }}>
            <Btn icon="refresh" variant="ghost" size="sm" onClick={refresh}>{t('common.refresh')}</Btn>
          </div>
        }
      />
      <Card padding={0}>
        <div style={{ padding: 18, borderBottom: '0.5px solid var(--ol-line)' }}>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
            <strong style={{ fontSize: 12 }}>{t('vocab.presets.title')}</strong>
            {presets.map(p => (
              <button
                key={p.id}
                onClick={() => togglePreset(p.id)}
                style={{
                  border: '0.5px solid var(--ol-line-strong)',
                  borderRadius: 999,
                  padding: '4px 10px',
                  fontSize: 12,
                  background: selectedPresetIds.includes(p.id) ? 'var(--ol-blue-soft)' : 'var(--ol-surface-2)',
                }}
              >
                {p.name}
              </button>
            ))}
            <Btn size="sm" variant="ghost" onClick={createPreset}>{t('vocab.presets.create')}</Btn>
            <Btn size="sm" variant="primary" onClick={applySelectedPresets}>{t('vocab.presets.apply')}</Btn>
          </div>
          <div style={{ fontSize: 11, color: 'var(--ol-ink-4)', marginTop: 10 }}>{t('vocab.presets.tip')}</div>
          {editingPresetId && (
            <div style={{ marginTop: 10, display: 'grid', gap: 8 }}>
              <input value={presetNameDraft} onChange={e => setPresetNameDraft(e.target.value)} placeholder={t('vocab.presets.namePlaceholder')} />
              <textarea value={presetPhrasesDraft} onChange={e => setPresetPhrasesDraft(e.target.value)} placeholder={t('vocab.presets.wordsPlaceholder')} rows={3} />
              <div style={{ display: 'flex', gap: 8 }}>
                <Btn size="sm" variant="primary" onClick={() => void savePreset()}>{t('vocab.presets.save')}</Btn>
                <Btn size="sm" variant="ghost" onClick={() => setEditingPresetId(null)}>{t('common.cancel')}</Btn>
              </div>
            </div>
          )}
          {!editingPresetId && presets.length > 0 && (
            <div style={{ marginTop: 8, display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              {presets.map(p => (
                <Btn key={`${p.id}-edit`} size="sm" variant="ghost" onClick={() => startEditPreset(p)}>
                  {t('vocab.presets.edit', { name: p.name })}
                </Btn>
              ))}
            </div>
          )}
        </div>
        <div style={{ padding: 18, borderBottom: '0.5px solid var(--ol-line)' }}>
          <div style={{ display: 'flex', gap: 8 }}>
            <input
              ref={inputRef}
              placeholder={t('vocab.placeholder')}
              onKeyDown={onKeyDown}
              style={{
                flex: 1, height: 36, padding: '0 12px',
                border: '0.5px solid var(--ol-line-strong)',
                borderRadius: 8, fontSize: 13,
                fontFamily: 'inherit', outline: 'none',
                background: 'var(--ol-surface-2)',
                transition: 'border-color 0.16s var(--ol-motion-quick), box-shadow 0.18s var(--ol-motion-soft), background 0.16s var(--ol-motion-quick)',
              }}
            />
            <Btn variant="primary" icon="plus" onClick={onAdd}>{t('common.add')}</Btn>
          </div>
          <div style={{ fontSize: 11, color: 'var(--ol-ink-4)', marginTop: 10 }}>
            {t('vocab.tip')}
          </div>
        </div>
        <div style={{ padding: 18, display: 'flex', flexWrap: 'wrap', gap: 8, minHeight: 80 }}>
          {loading && <div style={{ fontSize: 12, color: 'var(--ol-ink-4)' }}>{t('common.loading')}</div>}
          {!loading && error && (
            <div style={{ fontSize: 12, color: 'var(--ol-err)', lineHeight: 1.6 }}>
              {t('vocab.loadFailed', { err: error })}
            </div>
          )}
          {!loading && !error && entries.length === 0 && (
            <div style={{ fontSize: 12, color: 'var(--ol-ink-4)' }}>
              {t('vocab.empty')}
            </div>
          )}
          {!error && entries.map(e => (
            <VocabChip key={e.id} entry={e} onRemove={() => onRemove(e.id)} onToggle={() => onToggle(e)} />
          ))}
        </div>
      </Card>
      <style>{`
        @keyframes ol-chip-in {
          from { opacity: 0; transform: scale(.92); filter: blur(5px); }
          to   { opacity: 1; transform: scale(1); filter: blur(0); }
        }
      `}</style>
    </>
  );
}

interface VocabChipProps {
  entry: DictionaryEntry;
  onRemove: () => void;
  onToggle: () => void;
}

function VocabChip({ entry, onRemove, onToggle }: VocabChipProps) {
  const { t } = useTranslation();
  const enabled = entry.enabled;
  return (
    <span
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 6,
        padding: '5px 10px 5px 12px',
        borderRadius: 999,
        border: '0.5px solid var(--ol-line-strong)',
        background: enabled ? (entry.hits > 0 ? 'var(--ol-blue-soft)' : 'var(--ol-surface)') : 'var(--ol-surface-2)',
        opacity: enabled ? 1 : 0.55,
        fontSize: 12, color: 'var(--ol-ink)',
        fontFamily: 'var(--ol-font-mono)',
        transition: 'background 0.16s var(--ol-motion-quick), opacity 0.18s var(--ol-motion-soft), border-color 0.16s var(--ol-motion-quick)',
        animation: 'ol-chip-in 0.22s var(--ol-motion-spring)',
      }}
    >
      <button
        onClick={onToggle}
        title={enabled ? t('vocab.tipDisabled') : t('vocab.tipEnabled')}
        style={{ background: 'transparent', border: 0, padding: 0, color: 'inherit', fontFamily: 'inherit', cursor: 'default' }}
      >
        {entry.phrase}
      </button>
      <span
        style={{
          minWidth: 18, height: 18, padding: '0 5px',
          borderRadius: 999, fontSize: 10, fontWeight: 600,
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
          background: entry.hits > 0 && enabled ? 'var(--ol-blue)' : 'rgba(0,0,0,0.06)',
          color: entry.hits > 0 && enabled ? '#fff' : 'var(--ol-ink-4)',
          fontFamily: 'var(--ol-font-sans)',
        }}
      >{entry.hits}</span>
      <button
        onClick={onRemove}
        aria-label={t('vocab.removeAria')}
        style={{
          width: 14, height: 14, padding: 0, border: 0, borderRadius: 999,
          background: 'transparent', color: 'var(--ol-ink-4)',
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center', cursor: 'default',
        }}
      >
        <svg width="8" height="8" viewBox="0 0 8 8"><path d="M1 1l6 6M7 1l-6 6" stroke="currentColor" strokeWidth="1.4" /></svg>
      </button>
    </span>
  );
}
