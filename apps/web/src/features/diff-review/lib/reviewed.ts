const KEY = 'berth.diff.reviewed';
const SCOPE_LIMIT = 100;

export type Marks = Readonly<Record<string, string>>;

type Store = Record<string, Record<string, string>>;

function readStore(): Store {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}');
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? (parsed as Store) : {};
  } catch {
    return {};
  }
}

export function loadMarks(scope: string): Marks {
  return readStore()[scope] ?? {};
}

export function saveMarks(scope: string, marks: Marks): void {
  const store = readStore();
  delete store[scope];
  if (Object.keys(marks).length) store[scope] = { ...marks };
  const scopes = Object.keys(store);
  for (const old of scopes.slice(0, Math.max(0, scopes.length - SCOPE_LIMIT))) delete store[old];
  try {
    localStorage.setItem(KEY, JSON.stringify(store));
  } catch {
    return;
  }
}

export function fingerprint(text: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return `${text.length}:${(hash >>> 0).toString(16)}`;
}
