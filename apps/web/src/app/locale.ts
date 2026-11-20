import { setHostDecoder } from '../bridge';
import { bindLocale, type Locale } from '../shared/lib/i18n/i18n';
import { localizeHost } from '../shared/lib/i18n/hostText';
import { useSettings } from '../shared/config/settings/store';

export function initLocale(): () => void {
  const settings = useSettings();
  bindLocale({
    read: (): Locale => ((settings.values.value as Record<string, unknown>)['ui.language'] === 'ru' ? 'ru' : 'en'),
    write: (next) => settings.set('ui.language', next),
  });
  setHostDecoder(localizeHost);
  return () => undefined;
}

export function initBrowserLocale(): void {
  const locale: Locale = navigator.language.toLowerCase().startsWith('ru') ? 'ru' : 'en';
  bindLocale({ read: () => locale, write: () => undefined });
  setHostDecoder(localizeHost);
}
