import { acpPrompt, isAcpAgent, sendInput, type SessionRef } from './berth';

export function submitPrompt(session: SessionRef, text: string): Promise<void> {
  if ('Agent' in session && isAcpAgent(session.Agent)) return acpPrompt(session.Agent, text);
  return sendInput(session, `\x1b[200~${text}\x1b[201~`).then(() => sendInput(session, '\r'));
}
