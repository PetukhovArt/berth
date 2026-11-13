const TRACKER_KEY = /[A-Z][A-Z0-9]+-\d+/;

export function issueKeyOf(...texts: readonly string[]): string {
  for (const text of texts) {
    const key = TRACKER_KEY.exec(text.toUpperCase())?.[0];
    if (key) return key;
  }
  return '';
}
