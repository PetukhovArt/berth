import { addPromptQuote, sendInput, sessionKey, useSessionDock, type PromptQuote, type SessionRef } from '../../../entities/session';
import { showView, showViewInActivePanel, talkViewOf } from './sessionView';

export function quoteToPrompt(session: SessionRef, quote: PromptQuote, beside?: string): void {
  const key = sessionKey(session);
  const chat = talkViewOf(key) === 'chat';
  if (chat) addPromptQuote(session, quote);
  else void sendInput(session, `\x1b[200~${quote.text}\x1b[201~`);
  if (beside) useSessionDock().openWhenListed(session, beside);
  if (chat) showView(key, 'chat');
  else if (!beside) showViewInActivePanel('terminal');
}
