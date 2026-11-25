const foldCharacters = (text: string) => text.replace(/ς/g, 'σ').replace(/[‘’]/g, "'");

function foldSearchCase(text: string): string {
  const lowered = foldCharacters(text.toLowerCase());
  if (lowered.length === text.length) return lowered;
  let folded = '';
  for (const character of text) {
    const loweredCharacter = foldCharacters(character.toLowerCase());
    folded += loweredCharacter.length === character.length ? loweredCharacter : character;
  }
  return folded;
}

export function findOccurrences(text: string, query: string): number[] {
  const offsets: number[] = [];
  if (!text || !query) return offsets;
  const foldedText = foldSearchCase(text);
  const foldedQuery = foldSearchCase(query);
  for (let cursor = 0; cursor <= foldedText.length - foldedQuery.length; ) {
    const offset = foldedText.indexOf(foldedQuery, cursor);
    if (offset === -1) break;
    offsets.push(offset);
    cursor = offset + foldedQuery.length;
  }
  return offsets;
}

export function stepFindIndex(index: number, total: number, delta: number): number {
  if (total <= 0) return 0;
  const clamped = Number.isFinite(index) ? Math.min(Math.max(Math.trunc(index), 0), total - 1) : 0;
  return (((clamped + delta) % total) + total) % total;
}
