import { shallowRef, watch } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { BerthCallError, closeTerminal, describeError, useBerth } from './berth';
import { externalKey, useExternalSessions, type ExternalSession } from './external';
import { resumeConversation } from './orphans';

type Handoff = { state: 'pending' } | { state: 'failed'; message: string };

const STOP_TIMEOUT_MS = 15_000;
const handoffs = shallowRef<ReadonlyMap<string, Handoff>>(new Map());

function setHandoff(key: string, handoff: Handoff | null) {
  const next = new Map(handoffs.value);
  if (handoff) next.set(key, handoff);
  else next.delete(key);
  handoffs.value = next;
}

export function handoffOf(conversation: Pick<ExternalSession, 'harness' | 'id'>): Handoff | undefined {
  return handoffs.value.get(externalKey(conversation));
}

export function clearHandoff(conversation: Pick<ExternalSession, 'harness' | 'id'>): void {
  if (handoffs.value.get(externalKey(conversation))?.state === 'failed') setHandoff(externalKey(conversation), null);
}

export function canContinueAsAgent(conversation: ExternalSession | undefined): conversation is ExternalSession {
  return !!conversation && conversation.status !== 'running' && handoffOf(conversation)?.state !== 'pending';
}

export async function continueTerminalAsAgent(terminal: string, conversation: ExternalSession): Promise<string> {
  const key = externalKey(conversation);
  const worktree = useBerth()?.entities.value.terminals.get(terminal)?.worktree_id;
  setHandoff(key, { state: 'pending' });
  try {
    if (!worktree) throw new BerthCallError(t('workbench.external.terminalGone'));
    await closeTerminal(terminal);
    await whenStopped(key);
    const agent = await resumeConversation(conversation, worktree);
    setHandoff(key, null);
    return agent;
  } catch (e) {
    const message = t('workbench.external.asAgentFailed', { name: conversation.title, detail: describeError(e) });
    setHandoff(key, { state: 'failed', message });
    throw new BerthCallError(message);
  }
}

function whenStopped(key: string): Promise<void> {
  const external = useExternalSessions();
  const running = () => external?.anyByKey.value.get(key)?.running === true;
  if (!running()) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const finish = (stopped: boolean) => {
      clearTimeout(timer);
      stop();
      if (stopped) resolve();
      else reject(new BerthCallError(t('workbench.external.stillRunning')));
    };
    const timer = setTimeout(() => finish(false), STOP_TIMEOUT_MS);
    const stop = watch(running, (now) => {
      if (!now) finish(true);
    });
  });
}
