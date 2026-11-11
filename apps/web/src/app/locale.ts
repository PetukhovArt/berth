import { bindLocale, type Locale } from '../shared/lib/i18n/i18n';
import { useSettings } from '../shared/config/settings/store';

export function installLocale(): void {
  const settings = useSettings();
  bindLocale({
    read: (): Locale => ((settings.values.value as Record<string, unknown>)['ui.language'] === 'ru' ? 'ru' : 'en'),
    write: (next) => settings.set('ui.language', next),
  });
}
