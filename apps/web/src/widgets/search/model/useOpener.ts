import { computed } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { externalKey, openChannel, sessionKey, useBerth, useExternalSessions, useSessionDock, type ExternalSession, type MessageHit } from '../../../entities/session';
import { openCleanup } from '../../../features/cleanup';
import { openLeaf, type WorkNode } from '../../../features/work-tree';
import type { SearchRow } from './rows';

const SHORT_ID = 8;

type Conversation = Pick<ExternalSession, 'harness' | 'id' | 'title'>;

const ACTIONS: Readonly<Record<string, () => void>> = { cleanup: openCleanup };

export function useOpener() {
  const berth = useBerth();
  const external = useExternalSessions();
  const dock = useSessionDock();

  const owners = computed(() => {
    const byConversation = new Map<string, { id: string; name: string; archived: boolean }>();
    for (const agent of berth?.entities.value.agents.values() ?? []) if (agent.session_id) byConversation.set(`${agent.kind}:${agent.session_id}`, agent);
    return byConversation;
  });

  function conversationOfArchived(node: WorkNode): Conversation | null {
    const session = node.data?.session;
    const agent = session ? berth?.entities.value.agents.get(session.id) : undefined;
    if (!session || !agent?.session_id) return null;
    return agent.kind === 'claude' || agent.kind === 'codex' ? { harness: agent.kind, id: agent.session_id, title: session.name } : null;
  }

  function hitTitle(hit: MessageHit): string {
    const owner = owners.value.get(`${hit.harness}:${hit.session}`);
    const listed = external?.anyByKey.value.get(externalKey({ harness: hit.harness, id: hit.session }));
    return owner?.name ?? listed?.title ?? hit.title ?? `${hit.harness} ${hit.session.slice(0, SHORT_ID)}`;
  }

  function openHit(hit: MessageHit) {
    const ref: Conversation = { harness: hit.harness, id: hit.session, title: hitTitle(hit) };
    const owner = owners.value.get(`${hit.harness}:${hit.session}`);
    const node = owner && !owner.archived ? berth?.sessions.value.get(sessionKey({ Agent: owner.id })) : undefined;
    if (node) dock.openLinkedConversation(node, ref);
    else dock.openExternal(external?.anyByKey.value.get(externalKey(ref)) ?? ref);
  }

  function blockedReason(row: SearchRow): string | null {
    if (row.kind !== 'sessions' || row.entry.node.kind !== 'archived') return null;
    return conversationOfArchived(row.entry.node) ? null : t('search.blocked.archived');
  }

  function open(row: SearchRow) {
    if (row.kind === 'channels') openChannel(row.message.channel);
    else if (row.kind === 'messages') openHit(row.hit);
    else if (row.kind === 'files') dock.openFile(row.file.path, { worktreeId: dock.activeWorktree.value?.id });
    else if (row.kind === 'actions') ACTIONS[row.action.key]?.();
    else if (row.entry.node.kind === 'archived') {
      const conversation = conversationOfArchived(row.entry.node);
      if (conversation) dock.openExternal(conversation);
    } else openLeaf(row.entry.node);
  }

  return { hitTitle, blockedReason, open };
}
