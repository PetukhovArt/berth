import { sendInput, useSessionDock, type SessionRef } from '../../../entities/session';
import { showViewInActivePanel } from './sessionView';

export function quoteToPrompt(session: SessionRef, text: string, beside?: string): void {
  void sendInput(session, `\x1b[200~${text}\x1b[201~`);
  if (beside) useSessionDock().openWhenListed(session, beside);
  else showViewInActivePanel('terminal');
}
