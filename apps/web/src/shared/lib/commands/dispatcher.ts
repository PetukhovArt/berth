import { shallowRef } from 'vue';
import '../../../app/actions';
import { readContext } from './context';
import { chordFromEvent, isTypingChord, sequenceKey, type Chord } from './keys';
import { resolveSequence, shortcutsOf, startKeymap } from './keymap';
import { getAction, isAvailable, runAction } from './registry';

const SEQUENCE_TIMEOUT_MS = 1500;

type Capture = (chord: Chord) => void;

let capture: Capture | null = null;
const pending = shallowRef<readonly Chord[]>([]);
let timer: ReturnType<typeof setTimeout> | undefined;
let installed = false;

export function captureChords(handler: Capture): () => void {
  capture = handler;
  return () => {
    if (capture === handler) capture = null;
  };
}

export function pendingLeadsTo(actionId: string): boolean {
  if (!pending.value.length) return false;
  const prefix = `${sequenceKey(pending.value)} `;
  return shortcutsOf(actionId).some((shortcut) => shortcut.key.startsWith(prefix));
}

function swallow(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
}

function clearPending(): void {
  pending.value = [];
  clearTimeout(timer);
}

function exactAction(chords: readonly Chord[], context: ReturnType<typeof readContext>): string | null {
  const { exact } = resolveSequence(context, chords);
  const action = exact === null ? undefined : getAction(exact);
  return action && isAvailable(action, context) ? action.id : null;
}

function candidates(chord: Chord, typing: boolean): Chord[][] {
  if (!pending.value.length) return [[chord]];
  const continued = [...pending.value, chord];
  return typing ? [continued] : [continued, [chord]];
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.isComposing) return;
  const chord = chordFromEvent(event);
  if (!chord) return;
  if (capture) {
    swallow(event);
    capture(chord);
    return;
  }
  const context = readContext(event.target);
  const typing = isTypingChord(chord) && (context.inputFocus || context.terminalFocus);
  if (typing && !pending.value.length) return;
  if (pending.value.length && chord.code === 'Escape' && !chord.ctrl && !chord.alt && !chord.shift && !chord.meta) {
    clearPending();
    swallow(event);
    return;
  }
  for (const sequence of candidates(chord, typing)) {
    const { longer } = resolveSequence(context, sequence);
    const action = exactAction(sequence, context);
    if (longer) {
      swallow(event);
      clearTimeout(timer);
      pending.value = sequence;
      timer = setTimeout(() => {
        pending.value = [];
        if (action) runAction(action);
      }, SEQUENCE_TIMEOUT_MS);
      return;
    }
    if (action) {
      clearPending();
      swallow(event);
      runAction(action, context);
      return;
    }
  }
  if (pending.value.length) {
    clearPending();
    swallow(event);
  }
}

export function installKeymap(): void {
  if (installed) return;
  installed = true;
  startKeymap();
  window.addEventListener('keydown', onKeyDown, true);
}
