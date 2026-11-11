import type { MessageKey } from '../../lib/i18n/i18n';
import type { Params, PluralKeyPaths } from '../../lib/i18n/types';
import type { en } from '../../lib/i18n/en';

export type PlainKey = Exclude<MessageKey, PluralKeyPaths<typeof en>>;
type Translate = (key: PlainKey, params?: Params) => string;

let impl: Translate = (key) => key;

export function setTranslator(next: Translate): void {
  impl = next;
}

export function tr(key: PlainKey, params?: Params): string {
  return impl(key, params);
}
