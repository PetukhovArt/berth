import { computed, shallowRef, watch, type Ref } from 'vue';
import { MIN_MESSAGE_QUERY, useLoadedChannelMessages } from '../../../entities/session';
import { quickItems, useWorkTree, type QuickItem } from '../../../features/work-tree';
import type { MarkdownEntry } from '../../../shared/lib/markdownFiles';
import { rank, wordsOf } from './rank';
import { KINDS, SCOPE_KINDS, entityKindOf, type SearchAction, type SearchGroup, type SearchKind, type SearchRow, type SearchScope } from './rows';
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

export function useSearch(options: SearchOptions) {
  const { query, scope } = options;
  const { all } = useWorkTree();
  const loadedChannels = useLoadedChannelMessages();
  const opener = useOpener();
  const expanded = shallowRef<ReadonlySet<SearchKind>>(new Set());

  const text = computed(() => query.value.trim());
  const words = computed(() => wordsOf(text.value));
  watch([text, scope], () => (expanded.value = new Set()));
  const wantsMessages = computed(() => scope.value === 'all' || scope.value === 'messages');
  const transcripts = useTranscriptSearch(query, wantsMessages);
  const items = computed(() => quickItems(all.value, true));
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

  const messageRows = computed<SearchRow[]>(() => (transcripts.result.value?.hits ?? []).map((hit) => ({ id: `message:${hit.harness}:${hit.session}:${hit.key}`, kind: 'messages', hit })));

  const allRows = computed<Record<SearchKind, readonly SearchRow[]>>(() => ({ ...entityRows.value, files: fileRows.value, actions: actionRows.value, messages: messageRows.value, channels: channelRows.value }));

  const counts = computed(() => Object.fromEntries(KINDS.map((kind) => [kind, allRows.value[kind].length])) as Record<SearchKind, number>);
  const total = computed(() => SCOPE_KINDS.reduce((sum, kind) => sum + counts.value[kind], 0));

  const groups = computed<SearchGroup[]>(() =>
    KINDS.filter((kind) => scope.value === 'all' || scope.value === kind).flatMap((kind) => {
      const rows = allRows.value[kind];
      if (!rows.length) return [];
      const cap = options.limit(kind);
      const shown = expanded.value.has(kind) ? rows : rows.slice(0, cap);
      return [{ kind, total: rows.length, rows: shown, hidden: rows.length - shown.length }];
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
    counts,
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
    choose,
    hitTitle: opener.hitTitle,
    blockedReason: opener.blockedReason,
  };
}
