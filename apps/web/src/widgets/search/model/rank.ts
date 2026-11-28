import { wordScore } from '../../../shared/lib/fuzzy';

const TITLE_BOOST = 250;

export const wordsOf = (query: string): string[] => query.toLowerCase().split(/\s+/).filter(Boolean);

function scoreOf(words: readonly string[], text: string): number | null {
  let total = 0;
  for (const word of words) {
    const score = wordScore(word, text);
    if (score === null) return null;
    total += score;
  }
  return total;
}

export const titleMatches = (words: readonly string[], title: string): boolean => words.length > 0 && scoreOf(words, title.toLowerCase()) !== null;

export type Rankable = { text: string; title: string; at: number };

export function rank<T>(words: readonly string[], items: readonly T[], describe: (item: T) => Rankable): T[] {
  const scored: { item: T; score: number; at: number }[] = [];
  for (const item of items) {
    const { text, title, at } = describe(item);
    const total = scoreOf(words, text.toLowerCase());
    if (total === null) continue;
    const titled = titleMatches(words, title);
    scored.push({ item, score: total + (titled ? TITLE_BOOST * words.length : 0), at });
  }
  return scored.sort((a, b) => b.score - a.score || b.at - a.at).map((entry) => entry.item);
}
