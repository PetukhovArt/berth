import { shallowRef } from 'vue';
import { sessionKey, type SessionRef } from './berth';

export type PromptQuote = { label: string; text: string };

const NONE: readonly PromptQuote[] = [];
const quotes = shallowRef<ReadonlyMap<string, readonly PromptQuote[]>>(new Map());

function store(session: SessionRef, next: readonly PromptQuote[]): void {
  const map = new Map(quotes.value);
  if (next.length) map.set(sessionKey(session), next);
  else map.delete(sessionKey(session));
  quotes.value = map;
}

export function promptQuotesOf(session: SessionRef): readonly PromptQuote[] {
  return quotes.value.get(sessionKey(session)) ?? NONE;
}

export function addPromptQuote(session: SessionRef, quote: PromptQuote): void {
  store(session, [...promptQuotesOf(session).filter((entry) => entry.label !== quote.label), quote]);
}

export function removePromptQuote(session: SessionRef, label: string): void {
  store(session, promptQuotesOf(session).filter((entry) => entry.label !== label));
}

export function clearPromptQuotes(session: SessionRef): void {
  store(session, NONE);
}
