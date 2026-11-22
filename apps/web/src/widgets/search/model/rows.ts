import type { LoadedChannelMessage, MessageHit } from '../../../entities/session';
import type { QuickItem } from '../../../features/work-tree';
import type { MarkdownEntry } from '../../../shared/lib/markdownFiles';

export const SCOPE_KINDS = ['sessions', 'mr', 'tasks', 'messages', 'channels'] as const;
export const KINDS = ['sessions', 'mr', 'tasks', 'files', 'actions', 'messages', 'channels'] as const;

export type SearchKind = (typeof KINDS)[number];
export type SearchScope = 'all' | (typeof SCOPE_KINDS)[number];
export type SearchAction = { key: string; label: string };

export type ConversationRow = { id: string; kind: 'messages'; head: MessageHit; hits: readonly MessageHit[] };

export type SearchRow =
  | { id: string; kind: 'sessions' | 'mr' | 'tasks'; entry: QuickItem }
  | { id: string; kind: 'files'; file: MarkdownEntry }
  | { id: string; kind: 'actions'; action: SearchAction }
  | ConversationRow
  | { id: string; kind: 'channels'; message: LoadedChannelMessage };

export type SearchGroup = { kind: SearchKind; total: number; rows: readonly SearchRow[]; hidden: number };

export const entityKindOf = (nodeKind: string): 'sessions' | 'mr' | 'tasks' => {
  if (nodeKind === 'mr') return 'mr';
  return nodeKind === 'issue' ? 'tasks' : 'sessions';
};
