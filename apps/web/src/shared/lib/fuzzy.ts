const SEPARATORS = ' ›-_/.';

export function subsequenceScore(word: string, text: string): number | null {
  let matched = 0;
  let last = -2;
  let score = 0;
  for (let position = 0; position < text.length && matched < word.length; position++) {
    if (text[position] !== word[matched]) continue;
    score += position === last + 1 ? 3 : 1;
    if (position === 0 || SEPARATORS.includes(text[position - 1] ?? '')) score += 2;
    last = position;
    matched++;
  }
  return matched === word.length ? score : null;
}

const SUBSEQUENCE_MAX_WORD = 3;
const PREFIX_SCORE = 200;
const SUBSTRING_SCORE = 100;

export function wordScore(word: string, text: string): number | null {
  let best: number | null = null;
  for (let at = text.indexOf(word); at >= 0 && best !== PREFIX_SCORE; at = text.indexOf(word, at + 1)) {
    const boundary = at === 0 || SEPARATORS.includes(text[at - 1] ?? '');
    best = boundary ? PREFIX_SCORE : SUBSTRING_SCORE;
  }
  if (best !== null) return best;
  return word.length > SUBSEQUENCE_MAX_WORD ? null : subsequenceScore(word, text);
}

export function fuzzyScore(query: string, text: string): number | null {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const haystack = text.toLowerCase();
  let total = 0;
  for (const word of words) {
    const score = subsequenceScore(word, haystack);
    if (score === null) return null;
    total += score;
  }
  return total;
}
