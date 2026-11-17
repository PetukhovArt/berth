import { t } from '../../../shared/lib/i18n/i18n';
import { acknowledged, call, BerthCallError, type BerthReply } from './berth';

export type OrphanedSession = Extract<BerthReply, { OrphanedSessions: unknown }>['OrphanedSessions']['sessions'][number];

async function listResumable(project: string, includeLive: boolean): Promise<OrphanedSession[]> {
  const reply = await call({ ListOrphanedSessions: { project, include_live: includeLive } });
  if (!('OrphanedSessions' in reply)) throw new BerthCallError(t('agents.errors.expectedOrphans', { reply: Object.keys(reply).join('') }));
  return reply.OrphanedSessions.sessions;
}

export function listOrphans(project: string): Promise<OrphanedSession[]> {
  return listResumable(project, false);
}

export function listProjectConversations(project: string): Promise<OrphanedSession[]> {
  return listResumable(project, true);
}

export type ResumableConversation = { harness: 'claude' | 'codex'; id: string; title: string };

export async function resumeConversation(conversation: ResumableConversation, worktree: string): Promise<string> {
  const created = await acknowledged({ ResumeConversation: { worktree, kind: conversation.harness, session_id: conversation.id, name: conversation.title } });
  if (!created || !('Agent' in created)) throw new BerthCallError(t('agents.errors.noAgentCreated'));
  return created.Agent;
}

export async function resumeOrphan(sessionId: string, worktree: string): Promise<string> {
  const created = await acknowledged({ ResumeOrphanedSession: { session_id: sessionId, worktree } });
  if (!created || !('Agent' in created)) throw new BerthCallError(t('agents.errors.noAgentCreated'));
  return created.Agent;
}

export async function moveAgent(agent: string, worktree: string): Promise<void> {
  await acknowledged({ MoveAgent: { id: agent, worktree } });
}
