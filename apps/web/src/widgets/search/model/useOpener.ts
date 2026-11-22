import { computed } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { externalKey, openChannel, sessionKey, useBerth, useExternalSessions, useSessionDock, type ConversationHit, type ExternalSession } from '../../../entities/session';
import { openCleanup } from '../../../features/cleanup';
import { openLeaf, type WorkNode } from '../../../features/work-tree';
import type { SearchRow } from './rows';

const SHORT_ID = 8;

type Conversation = Pick<ExternalSession, 'harness' | 'id' | 'title'>;

const conversationKeyOfAgent = (agent: { id: string; kind: string; session_id?: string | null }): string | null => {
  if (agent.kind === 'acp') return `acp:${agent.id}`;
  return agent.session_id ? `${agent.kind}:${agent.session_id}` : null;
};

const ACTIONS: Readonly<Record<string, () => void>> = { cleanup: openCleanup };

export function useOpener() {
  const berth = useBerth();
  const external = useExternalSessions();
  const dock = useSessionDock();

  const owners = computed(() => {
    const byConversation = new Map<string, { id: string; name: string; archived: boolean }>();
    for (const agent of berth?.entities.value.agents.values() ?? []) {
      const key = conversationKeyOfAgent(agent);
      if (key) byConversation.set(key, agent);
    }
    return byConversation;
  });

  function conversationOfArchived(node: WorkNode): Conversation | null {
    const session = node.data?.session;
    const agent = session ? berth?.entities.value.agents.get(session.id) : undefined;
    if (!session || !agent?.session_id) return null;
    return agent.kind === 'claude' || agent.kind === 'codex' ? { harness: agent.kind, id: agent.session_id, title: session.name } : null;
  }

  function conversationKeyOf(node: WorkNode): string | null {
    const { session, external, linked } = node.data ?? {};
    if (external) return `${external.harness}:${external.id}`;
    const agent = session ? berth?.entities.value.agents.get(session.id) : undefined;
    const own = agent ? conversationKeyOfAgent(agent) : null;
    if (own) return own;
    return linked ? `${linked.harness}:${linked.id}` : null;
  }

  function hitTitle(found: ConversationHit): string {
    const owner = owners.value.get(`${found.harness}:${found.session}`);
    const listed = found.harness === 'acp' ? undefined : external?.anyByKey.value.get(externalKey({ harness: found.harness, id: found.session }));
    return owner?.name ?? listed?.title ?? found.title ?? `${found.harness} ${found.session.slice(0, SHORT_ID)}`;
  }

  function openConversation(found: ConversationHit) {
    const owner = owners.value.get(`${found.harness}:${found.session}`);
    const node = owner && !owner.archived ? berth?.sessions.value.get(sessionKey({ Agent: owner.id })) : undefined;
    if (found.harness === 'acp') {
      if (node) dock.openSession(node);
      return;
    }
    const ref: Conversation = { harness: found.harness, id: found.session, title: hitTitle(found) };
    if (node) dock.openLinkedConversation(node, ref);
    else dock.openExternal(external?.anyByKey.value.get(externalKey(ref)) ?? ref);
  }

  function blockedReason(row: SearchRow): string | null {
    if (row.kind === 'messages' && row.conversation.harness === 'acp') {
      const owner = owners.value.get(`acp:${row.conversation.session}`);
      if (!owner) return t('search.blocked.gone');
      return owner.archived ? t('search.blocked.archived') : null;
    }
    if (row.kind !== 'sessions' || row.entry.node.kind !== 'archived') return null;
    return conversationOfArchived(row.entry.node) ? null : t('search.blocked.archived');
  }

  function open(row: SearchRow) {
    if (row.kind === 'channels') openChannel(row.message.channel);
    else if (row.kind === 'messages') openConversation(row.conversation);
    else if (row.kind === 'files') dock.openFile(row.file.path, { worktreeId: dock.activeWorktree.value?.id });
    else if (row.kind === 'actions') ACTIONS[row.action.key]?.();
    else if (row.entry.node.kind === 'archived') {
      const conversation = conversationOfArchived(row.entry.node);
      if (conversation) dock.openExternal(conversation);
    } else openLeaf(row.entry.node);
  }

  return { hitTitle, blockedReason, conversationKeyOf, open };
}
