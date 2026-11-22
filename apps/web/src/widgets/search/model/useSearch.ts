import { computed, shallowRef, watch, type Ref } from 'vue';
import { MIN_MESSAGE_QUERY, useLoadedChannelMessages, type MessageHit } from '../../../entities/session';
import { quickItems, useWorkTree, type QuickItem } from '../../../features/work-tree';
import { t } from '../../../shared/lib/i18n/i18n';
import type { MarkdownEntry } from '../../../shared/lib/markdownFiles';
import { rank, wordsOf } from './rank';
import { KINDS, entityKindOf, type ConversationRow, type SearchAction, type SearchGroup, type SearchKind, type SearchRow, type SearchScope } from './rows';
import { useOpener } from './useOpener';
import { useTranscriptSearch } from './useTranscriptSearch';

type SearchOptions = {
  query: Ref<string>;
  scope: Ref<SearchScope>;
  limit: (kind: SearchKind) => number;
  browse?: boolean;
  files?: Ref<readonly MarkdownEntry[]>;
  actions?: Ref<readonly SearchAction[]>;
};

const ARCHIVED_KINDS = new Set(['archived', 'archived-external']);

const timeOf = (value: number | string | null | undefined): number => {
  if (value === null || value === undefined) return 0;
  return typeof value === 'number' ? value : Date.parse(value) || 0;
};

const entryTime = (entry: QuickItem): number => entry.at ?? timeOf(entry.node.data?.item?.updated_at ?? entry.node.data?.issue?.updated);

function withoutPlaceholder(items: readonly QuickItem[]): QuickItem[] {
  const placeholder = t('workbench.notInProject');
  const lead = `${placeholder.toLowerCase()} › `;
  return items.map((item) => {
    if (!item.crumbs.includes(placeholder)) return item;
    return { ...item, crumbs: item.crumbs.filter((crumb) => crumb !== placeholder), search: item.search.replace(lead, '') };
  });
}

const conversationKeyOf = (hit: MessageHit): string => `${hit.harness}:${hit.session}`;

function conversationsOf(hits: readonly MessageHit[]): ConversationRow[] {
  const byConversation = new Map<string, MessageHit[]>();
  for (const hit of hits) {
    const id = `message:${hit.harness}:${hit.session}`;
    const known = byConversation.get(id);
    if (known) known.push(hit);
    else byConversation.set(id, [hit]);
  }
  const rows: ConversationRow[] = [];
  for (const [id, matches] of byConversation) {
    const [head] = matches;
    if (head) rows.push({ id, kind: 'messages', head, hits: matches });
  }
  return rows;
}

export function useSearch(options: SearchOptions) {
  const { query, scope } = options;
  const { all } = useWorkTree();
  const loadedChannels = useLoadedChannelMessages();
  const opener = useOpener();
  const expanded = shallowRef<ReadonlySet<SearchKind>>(new Set());

  const text = computed(() => query.value.trim());
  const words = computed(() => wordsOf(text.value));
  watch([text, scope], () => (expanded.value = new Set()));
  const wantsMessages = computed(() => scope.value === 'all' || scope.value === 'sessions' || scope.value === 'messages');
  const transcripts = useTranscriptSearch(query);
  const items = computed(() => withoutPlaceholder(quickItems(all.value, true)));
  const hasText = computed(() => text.value.length > 0);
  const listing = computed(() => hasText.value || options.browse === true);

  const entityRows = computed(() => {
    const out: Record<'sessions' | 'mr' | 'tasks', SearchRow[]> = { sessions: [], mr: [], tasks: [] };
    if (!listing.value) return out;
    const pool = hasText.value ? items.value : items.value.filter((entry) => !ARCHIVED_KINDS.has(entry.node.kind));
    for (const entry of rank(words.value, pool, (item) => ({ text: item.search, title: item.node.label, at: entryTime(item) }))) {
      const kind = entityKindOf(entry.node.kind);
      out[kind].push({ id: entry.key, kind, entry });
    }
    return out;
  });

  const fileRows = computed<SearchRow[]>(() => {
    if (!hasText.value || scope.value !== 'all') return [];
    return rank(words.value, options.files?.value ?? [], (file) => ({ text: file.rel, title: file.name, at: 0 })).map((file) => ({ id: `file:${file.path}`, kind: 'files', file }));
  });

  const actionRows = computed<SearchRow[]>(() => {
    if (!hasText.value || scope.value !== 'all') return [];
    return rank(words.value, options.actions?.value ?? [], (action) => ({ text: action.label, title: action.label, at: 0 })).map((action) => ({ id: `action:${action.key}`, kind: 'actions', action }));
  });

  const channelRows = computed<SearchRow[]>(() => {
    if (!hasText.value) return [];
    return rank(words.value, loadedChannels.value, (message) => ({ text: `${message.content} ${message.authorName} ${message.channel}`, title: message.content, at: message.createdAt })).map((message) => ({
      id: `channel:${message.channel}:${message.seq}`,
      kind: 'channels',
      message,
    }));
  });

  const messageRows = computed(() => conversationsOf(transcripts.result.value?.hits ?? []));

  const allRows = computed<Record<SearchKind, readonly SearchRow[]>>(() => ({ ...entityRows.value, files: fileRows.value, actions: actionRows.value, messages: messageRows.value, channels: channelRows.value }));

  const counts = computed(() => Object.fromEntries(KINDS.map((kind) => [kind, allRows.value[kind].length])) as Record<SearchKind, number>);

  function chipCount(id: SearchScope): number | undefined {
    if (!hasText.value) return undefined;
    if (id === 'all') return total.value;
    if (id === 'messages' && transcripts.result.value === null) return undefined;
    return counts.value[id];
  }

  const capped = (kind: SearchKind, rows: readonly SearchRow[]) => (expanded.value.has(kind) ? rows : rows.slice(0, options.limit(kind)));

  const folds = computed(() => {
    const into = new Map<string, ConversationRow>();
    const claimed = new Set<string>();
    const conversations = new Map(messageRows.value.map((row) => [conversationKeyOf(row.head), row]));
    for (const row of allRows.value.sessions) {
      if (row.kind !== 'sessions') continue;
      const key = opener.conversationKeyOf(row.entry.node);
      const conversation = key ? conversations.get(key) : undefined;
      if (!conversation || claimed.has(conversation.id)) continue;
      into.set(row.id, conversation);
      claimed.add(conversation.id);
    }
    return { into, claimed };
  });

  const total = computed(() => KINDS.reduce((sum, kind) => sum + counts.value[kind], 0) - folds.value.claimed.size);

  const groups = computed<SearchGroup[]>(() =>
    KINDS.filter((kind) => scope.value === 'all' || scope.value === kind).flatMap((kind) => {
      const unfolded = kind === 'messages' && scope.value !== 'messages';
      const rows = unfolded ? messageRows.value.filter((row) => !folds.value.claimed.has(row.id)) : allRows.value[kind];
      if (!rows.length) return [];
      const shown = capped(kind, rows);
      return [{ kind, total: rows.length, rows: shown, hidden: rows.length - shown.length, folded: unfolded && folds.value.claimed.size > 0 }];
    }),
  );

  const rows = computed(() => groups.value.flatMap((group) => group.rows));
  const tooShort = computed(() => wantsMessages.value && text.value.length > 0 && text.value.length < MIN_MESSAGE_QUERY);
  const waiting = computed(() => wantsMessages.value && transcripts.busy.value && transcripts.result.value === null);
  const nothing = computed(() => rows.value.length === 0 && !waiting.value && !transcripts.busy.value && !transcripts.error.value);
  const progress = computed(() => {
    const found = wantsMessages.value ? transcripts.result.value : null;
    return found && found.indexed < found.total ? found : null;
  });

  function showAll(kind: SearchKind) {
    expanded.value = new Set(expanded.value).add(kind);
  }

  function choose(row: SearchRow | undefined): boolean {
    if (!row || opener.blockedReason(row)) return false;
    opener.open(row);
    return true;
  }

  return {
    text,
    chipCount,
    total,
    groups,
    rows,
    tooShort,
    searching: waiting,
    nothing,
    progress,
    messageError: computed(() => (wantsMessages.value ? transcripts.error.value : null)),
    messagesStale: transcripts.stale,
    retryMessages: transcripts.retry,
    showAll,
    groupLabel: (group: SearchGroup): string => (group.folded ? t('search.otherConversations') : t(`search.scope.${group.kind}`)),
    foldedInto: (row: SearchRow): ConversationRow | undefined => (scope.value === 'messages' ? undefined : folds.value.into.get(row.id)),
    choose,
    hitTitle: opener.hitTitle,
    blockedReason: opener.blockedReason,
  };
}
