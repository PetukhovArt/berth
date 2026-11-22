import { invoke } from '../../../bridge';
import type { Harness } from './external';

export type MessageHit = { harness: Harness; session: string; key: string; role: 'user' | 'assistant'; at: string | null; snippet: string };
export type MessageSearch = { hits: MessageHit[]; indexed: number; total: number };

export const MIN_MESSAGE_QUERY = 3;

export function searchMessages(query: string, limit: number): Promise<MessageSearch> {
  return invoke<MessageSearch>('search_messages', { query, limit });
}
