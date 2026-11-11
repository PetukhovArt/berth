import type { Terminal } from '@xterm/xterm';
import { onlyPrimaryPressed } from '../shared/lib/platform';

type Kind = 'copy' | 'paste';

const NATIVE_GRACE_MS = 250;
const LATE_NATIVE_MS = 1500;

export type TerminalClipboard = { handleKey: (event: KeyboardEvent) => boolean | null; dispose: () => void };

export function terminalClipboard(term: Terminal, element: HTMLElement): TerminalClipboard {
  let waiting: { kind: Kind; timer: ReturnType<typeof setTimeout> } | null = null;
  let fallback: { kind: Kind; at: number } | null = null;

  const onNative = (event: Event) => {
    const kind = event.type as Kind;
    if (fallback?.kind === kind && Date.now() - fallback.at < LATE_NATIVE_MS) {
      fallback = null;
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
    if (waiting?.kind === kind) {
      clearTimeout(waiting.timer);
      waiting = null;
    }
  };
  element.addEventListener('copy', onNative, true);
  element.addEventListener('paste', onNative, true);

  const expect = (kind: Kind, run: () => Promise<void>) => {
    if (waiting) clearTimeout(waiting.timer);
    fallback = null;
    waiting = {
      kind,
      timer: setTimeout(() => {
        waiting = null;
        fallback = { kind, at: Date.now() };
        void run().catch(() => undefined);
      }, NATIVE_GRACE_MS),
    };
  };

  const paste = async () => {
    const text = await navigator.clipboard.readText();
    if (text) term.paste(text);
  };

  return {
    handleKey: (event) => {
      if (event.type !== 'keydown' || !onlyPrimaryPressed(event)) return null;
      if (event.code === 'KeyV') {
        expect('paste', paste);
        return false;
      }
      if (event.code === 'KeyC' && term.hasSelection()) {
        const text = term.getSelection();
        expect('copy', () => navigator.clipboard.writeText(text));
        return false;
      }
      return null;
    },
    dispose: () => {
      if (waiting) clearTimeout(waiting.timer);
      element.removeEventListener('copy', onNative, true);
      element.removeEventListener('paste', onNative, true);
    },
  };
}
