import i18n from '../i18n';
import type { HotkeyBinding, HotkeyTrigger } from './types';

export function getHotkeyTriggerLabel(trigger: HotkeyTrigger | null | undefined): string {
  if (!trigger) return i18n.t('hotkey.fallback');
  return i18n.t(`hotkey.triggers.${trigger}`);
}

export function getHotkeyStartStopLabel(binding: HotkeyBinding | null | undefined): string {
  const trigger = getHotkeyTriggerLabel(binding?.trigger);
  const suffix = binding?.mode === 'hold'
    ? i18n.t('hotkey.modeHoldSuffix')
    : i18n.t('hotkey.modeToggleSuffix');
  return `${trigger}${suffix}`;
}

export function getHotkeyUsageHint(binding: HotkeyBinding | null | undefined): string {
  const trigger = getHotkeyTriggerLabel(binding?.trigger);
  return binding?.mode === 'hold'
    ? i18n.t('hotkey.usageHold', { trigger })
    : i18n.t('hotkey.usageToggle', { trigger });
}
