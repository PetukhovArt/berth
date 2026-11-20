import { t } from './i18n';

const MARK = String.fromCharCode(0x1e);
const END = String.fromCharCode(0x1f);
const CODED = new RegExp(`${MARK}([^${END}]*)${END}`, 'g');

type Coded = { code: string; params: Record<string, string> };

function translate(code: string, params: Record<string, string>): string {
  const key = `host.${code}`;
  const text = t(key as never, params);
  return text === key ? `${code} ${JSON.stringify(params)}` : text;
}

export function hostText(text: string): string;
export function hostText(text: string | null | undefined): string | null | undefined;
export function hostText(text: string | null | undefined): string | null | undefined {
  if (!text?.includes(MARK)) return text;
  return text.replace(CODED, (match, body: string) => {
    try {
      const { code, params } = JSON.parse(body) as Partial<Coded>;
      if (typeof code !== 'string' || typeof params !== 'object' || params === null) return match;
      const values = Object.fromEntries(Object.entries(params).map(([name, value]) => [name, hostText(value)]));
      return translate(code, values);
    } catch {
      return match;
    }
  });
}

export function localizeHost<T>(value: T): T {
  if (typeof value === 'string') return hostText(value) as T;
  if (Array.isArray(value)) {
    let changed = false;
    const next = value.map((item: unknown) => {
      const mapped = localizeHost(item);
      if (mapped !== item) changed = true;
      return mapped;
    });
    return (changed ? next : value) as T;
  }
  if (value && typeof value === 'object' && Object.getPrototypeOf(value) === Object.prototype) {
    const source = value as Record<string, unknown>;
    let next: Record<string, unknown> | null = null;
    for (const [key, item] of Object.entries(source)) {
      const mapped = localizeHost(item);
      if (mapped !== item) (next ??= { ...source })[key] = mapped;
    }
    return (next ?? value) as T;
  }
  return value;
}
