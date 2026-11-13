import { onBeforeUnmount, shallowRef } from 'vue';

interface RecognitionResult {
  readonly isFinal: boolean;
  readonly 0: { readonly transcript: string };
}
interface RecognitionEvent {
  readonly resultIndex: number;
  readonly results: ArrayLike<RecognitionResult>;
}
interface Recognition {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((event: RecognitionEvent) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
}
type RecognitionCtor = new () => Recognition;

const CANCEL_SLIDE_PX = 80;
const MIN_HOLD_MS = 400;
const PAUSE_MS = 1600;

function recognitionCtor(): RecognitionCtor | null {
  const scope = window as unknown as { SpeechRecognition?: RecognitionCtor; webkitSpeechRecognition?: RecognitionCtor };
  return scope.SpeechRecognition ?? scope.webkitSpeechRecognition ?? null;
}

export type HoldToTalkIssue = 'denied' | 'short' | 'empty' | 'failed';

export function useHoldToTalk(options: { lang: () => string; onSend: (text: string) => void | Promise<void>; onIssue: (issue: HoldToTalkIssue) => void; onSpeech?: () => void; onSettled?: () => void }) {
  const supported = recognitionCtor() !== null;
  const holding = shallowRef(false);
  const cancelling = shallowRef(false);
  const transcript = shallowRef('');
  let recognition: Recognition | null = null;
  let origin = { x: 0, y: 0 };
  let startedAt = 0;
  let finalText = '';
  let released = false;
  let aborted = false;
  let failure: HoldToTalkIssue | null = null;
  let handsFree = false;
  let pauseTimer: ReturnType<typeof setTimeout> | undefined;

  function reset() {
    clearTimeout(pauseTimer);
    holding.value = false;
    cancelling.value = false;
    transcript.value = '';
    recognition = null;
  }

  function finish() {
    const text = (finalText + ' ' + transcript.value).trim();
    const issue = failure;
    const cancel = aborted;
    const heldFor = Date.now() - startedAt;
    const free = handsFree;
    reset();
    if (cancel) return;
    if (issue) options.onIssue(issue);
    else if (!text) {
      if (!free) options.onIssue(heldFor < MIN_HOLD_MS ? 'short' : 'empty');
    } else void options.onSend(text);
    options.onSettled?.();
  }

  function begin(free: boolean) {
    const Ctor = recognitionCtor();
    if (!Ctor || holding.value) return;
    handsFree = free;
    startedAt = Date.now();
    finalText = '';
    released = false;
    aborted = false;
    failure = null;
    transcript.value = '';
    holding.value = true;
    const next = new Ctor();
    next.lang = options.lang();
    next.continuous = true;
    next.interimResults = true;
    next.onresult = (e) => {
      options.onSpeech?.();
      let interim = '';
      for (let i = e.resultIndex; i < e.results.length; i += 1) {
        const result = e.results[i]!;
        if (result.isFinal) finalText = (finalText + ' ' + result[0].transcript).trim();
        else interim += result[0].transcript;
      }
      transcript.value = interim;
      if (handsFree) {
        clearTimeout(pauseTimer);
        pauseTimer = setTimeout(() => {
          if (recognition !== next || !(finalText || transcript.value)) return;
          released = true;
          holding.value = false;
          next.stop();
        }, PAUSE_MS);
      }
    };
    next.onerror = (e) => {
      if (e.error === 'not-allowed' || e.error === 'service-not-allowed') failure = 'denied';
      else if (e.error !== 'no-speech' && e.error !== 'aborted') failure = 'failed';
    };
    next.onend = () => {
      if (recognition !== next) return;
      if (released || failure) finish();
      else if (holding.value) next.start();
    };
    recognition = next;
    try {
      next.start();
    } catch {
      failure = 'failed';
      finish();
    }
  }

  function start(event: PointerEvent) {
    if (!recognitionCtor() || holding.value) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement | null)?.setPointerCapture(event.pointerId);
    origin = { x: event.clientX, y: event.clientY };
    begin(false);
  }

  function listen() {
    begin(true);
  }

  function stopListening() {
    if (!recognition) return;
    aborted = true;
    recognition.abort();
    finish();
  }

  function move(event: PointerEvent) {
    if (!holding.value) return;
    cancelling.value = Math.hypot(event.clientX - origin.x, event.clientY - origin.y) > CANCEL_SLIDE_PX;
  }

  function release(event: PointerEvent) {
    if (!holding.value || !recognition) return;
    (event.currentTarget as HTMLElement | null)?.releasePointerCapture?.(event.pointerId);
    released = true;
    holding.value = false;
    if (event.type === 'pointercancel' || cancelling.value) {
      aborted = true;
      recognition.abort();
      return finish();
    }
    recognition.stop();
  }

  onBeforeUnmount(() => {
    aborted = true;
    recognition?.abort();
  });

  return { supported, holding, cancelling, transcript, start, move, release, listen, stopListening };
}
