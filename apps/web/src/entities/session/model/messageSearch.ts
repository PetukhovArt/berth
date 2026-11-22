import { invoke } from '../../../bridge';
import type { Harness } from './external';

type MessageSnippet = {
  key: string;
  role: 'user' | 'assistant';
  at: string | null;
  snippet: string;
};
export type ConversationHit = {
  harness: Harness | 'acp';
  session: string;
  title: string | null;
  cwd: string | null;
  branch: string | null;
  matches: number;
  at: string | null;
  snippets: MessageSnippet[];
};
export type ConversationSearch = { conversations: ConversationHit[]; total: number; indexed: number; total_files: number; remote: boolean };

export const MIN_MESSAGE_QUERY = 3;

export const isSearchableMessageQuery = (text: string): boolean => text.split(/\s+/).some((word) => Array.from(word).length >= MIN_MESSAGE_QUERY);

export function searchConversations(query: string, limit: number): Promise<ConversationSearch> {
  return invoke<ConversationSearch>('search_conversations', { query, limit });
}
