const STORE_KEY = 'berth.dock.openSessions';

export type OpenTabs = { keys: string[]; active: string | null };

export function loadOpenTabs(): OpenTabs | null {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? 'null');
    if (!value || typeof value !== 'object' || !('keys' in value) || !Array.isArray(value.keys)) return null;
    const keys = value.keys.filter((key): key is string => typeof key === 'string');
    const active = 'active' in value && typeof value.active === 'string' ? value.active : null;
    return keys.length ? { keys, active } : null;
  } catch {
    return null;
  }
}

export function saveOpenTabs(tabs: OpenTabs | null): void {
  if (tabs?.keys.length) localStorage.setItem(STORE_KEY, JSON.stringify(tabs));
  else localStorage.removeItem(STORE_KEY);
}
