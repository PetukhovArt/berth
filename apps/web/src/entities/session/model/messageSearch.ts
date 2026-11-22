import { invoke } from '../../../bridge';
import type { Harness } from './external';

export type MessageHit = {
  harness: Harness | 'acp';
  session: string;
  key: string;
  role: 'user' | 'assistant';
  at: string | null;
  snippet: string;
  title: string | null;
  cwd: string | null;
  branch: string | null;
};
export type MessageSearch = { hits: MessageHit[]; indexed: number; total: number };

export const MIN_MESSAGE_QUERY = 3;

export const isSearchableMessageQuery = (text: string): boolean => text.split(/\s+/).some((word) => Array.from(word).length >= MIN_MESSAGE_QUERY);

export function searchMessages(query: string, limit: number): Promise<MessageSearch> {
  return invoke<MessageSearch>('search_messages', { query, limit });
}
