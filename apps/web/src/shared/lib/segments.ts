export type Segment = { text: string; match: boolean };

const OPEN = '\u0001';
const CLOSE = '\u0002';

const plain = (text: string): Segment => ({ text: text.replaceAll(CLOSE, ''), match: false });

export const stripMarks = (text: string): string => text.replaceAll(OPEN, '').replaceAll(CLOSE, '');

const ELLIPSIS = '…';
const PARTIAL_WORD = 16;

export function wholeWordStart(text: string): string {
  if (!text.startsWith(ELLIPSIS)) return text;
  const rest = text.slice(ELLIPSIS.length);
  const gap = rest.search(/\s/);
  if (gap < 0 || gap > PARTIAL_WORD || rest.slice(0, gap).includes(OPEN)) return text;
  return `${ELLIPSIS}${rest.slice(gap).trimStart()}`;
}

export function markedSegments(text: string): Segment[] {
  const out: Segment[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    const open = text.indexOf(OPEN, cursor);
    if (open < 0) break;
    if (open > cursor) out.push(plain(text.slice(cursor, open)));
    const close = text.indexOf(CLOSE, open + 1);
    const end = close < 0 ? text.length : close;
    if (end > open + 1) out.push({ text: text.slice(open + 1, end), match: true });
    cursor = close < 0 ? text.length : close + 1;
  }
  if (cursor < text.length) out.push(plain(text.slice(cursor)));
  return out;
}

export function wordSegments(text: string, query: string): Segment[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const lower = text.toLowerCase();
  if (!text) return [];
  if (!words.length || lower.length !== text.length) return [{ text, match: false }];
  const ranges: [number, number][] = [];
  for (const word of words) {
    for (let at = lower.indexOf(word); at >= 0; at = lower.indexOf(word, at + word.length)) ranges.push([at, at + word.length]);
  }
  ranges.sort((a, b) => a[0] - b[0]);
  const out: Segment[] = [];
  let cursor = 0;
  for (const [start, end] of ranges) {
    if (end <= cursor) continue;
    const from = Math.max(start, cursor);
    if (from > cursor) out.push({ text: text.slice(cursor, from), match: false });
    out.push({ text: text.slice(from, end), match: true });
    cursor = end;
  }
  if (cursor < text.length) out.push({ text: text.slice(cursor), match: false });
  return out;
}
