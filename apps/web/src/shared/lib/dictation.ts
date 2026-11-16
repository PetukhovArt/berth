import { onBeforeUnmount, shallowRef } from 'vue';
import { nativeEngine } from './nativeVoice';
import type { DictationEngine, HoldToTalkIssue, Take } from './speechEngine';

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

export type { HoldToTalkIssue };

type DictationOptions = {
  onSend: (text: string) => void | Promise<void>;
  onIssue: (issue: HoldToTalkIssue) => void;
  onSpeech?: () => void;
  onWoke?: () => void;
  onSettled?: () => void;
  wakePhrase?: () => string;
};

export function speechLang(): string {
  return document.documentElement.lang === 'ru' ? 'ru-RU' : 'en-US';
}

const browserEngine: DictationEngine = {
  supported: recognitionCtor() !== null,
  open(options, events) {
    const Ctor = recognitionCtor();
    if (!Ctor) {
      queueMicrotask(() => events.end('', 'failed'));
      return { stop: () => undefined, abort: () => undefined };
    }
    const recognition = new Ctor();
    let finalText = '';
    let interim = '';
    let stopped = false;
    let failure: HoldToTalkIssue | null = null;
    let pauseTimer: ReturnType<typeof setTimeout> | undefined;
    recognition.lang = speechLang();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.onresult = (e) => {
      events.speech();
      let next = '';
      for (let i = e.resultIndex; i < e.results.length; i += 1) {
        const result = e.results[i]!;
        if (result.isFinal) finalText = (finalText + ' ' + result[0].transcript).trim();
        else next += result[0].transcript;
      }
      interim = next;
      events.interim(next);
      if (!options.handsFree) return;
      clearTimeout(pauseTimer);
      pauseTimer = setTimeout(() => {
        if (stopped || !(finalText || interim)) return;
        stopped = true;
        recognition.stop();
      }, PAUSE_MS);
    };
    recognition.onerror = (e) => {
      if (e.error === 'not-allowed' || e.error === 'service-not-allowed') failure = 'denied';
      else if (e.error !== 'no-speech' && e.error !== 'aborted') failure = 'failed';
    };
    recognition.onend = () => {
      if (!stopped && !failure) return recognition.start();
      clearTimeout(pauseTimer);
      events.end((finalText + ' ' + interim).trim(), failure);
    };
    try {
      recognition.start();
    } catch {
      recognition.onend = null;
      queueMicrotask(() => events.end('', 'failed'));
    }
    return {
      stop: () => {
        stopped = true;
        recognition.stop();
      },
      abort: () => {
        clearTimeout(pauseTimer);
        recognition.onend = null;
        recognition.abort();
      },
    };
  },
};

const engine: DictationEngine = nativeEngine ?? browserEngine;

export function useHoldToTalk(options: DictationOptions) {
  const dictation = createDictation(options);
  onBeforeUnmount(dictation.stopListening);
  return dictation;
}

export function createDictation(options: DictationOptions) {
  const supported = engine.supported;
  const holding = shallowRef(false);
  const recognising = shallowRef(false);
  const cancelling = shallowRef(false);
  const transcript = shallowRef('');
  let take: Take | null = null;
  let owner: object | null = null;
  let origin = { x: 0, y: 0 };
  let startedAt = 0;
  let handsFree = false;

  function reset() {
    owner = null;
    take = null;
    holding.value = false;
    recognising.value = false;
    cancelling.value = false;
    transcript.value = '';
  }

  function finish(text: string, issue: HoldToTalkIssue | null) {
    const heldFor = Date.now() - startedAt;
    const free = handsFree;
    reset();
    if (issue) options.onIssue(issue);
    else if (!text) {
      if (!free) options.onIssue(heldFor < MIN_HOLD_MS ? 'short' : 'empty');
    } else void options.onSend(text);
    options.onSettled?.();
  }

  function begin(free: boolean) {
    if (!supported || owner) return;
    handsFree = free;
    startedAt = Date.now();
    transcript.value = '';
    holding.value = true;
    const token = {};
    owner = token;
    const opened = engine.open(
      { handsFree: free, wakePhrase: free ? (options.wakePhrase?.() ?? '') : '' },
      {
        woke: () => {
          if (owner === token) options.onWoke?.();
        },
        speech: () => {
          if (owner === token) options.onSpeech?.();
        },
        interim: (text) => {
          if (owner === token) transcript.value = text;
        },
        end: (text, issue) => {
          if (owner === token) finish(text, issue);
        },
      },
    );
    if (owner === token) take = opened;
  }

  function abort() {
    const current = take;
    reset();
    current?.abort();
  }

  function settle() {
    if (!holding.value || !take) return;
    holding.value = false;
    recognising.value = true;
    take.stop();
  }

  function start(event: PointerEvent) {
    if (!supported || owner) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement | null)?.setPointerCapture(event.pointerId);
    origin = { x: event.clientX, y: event.clientY };
    begin(false);
  }

  function listen() {
    begin(true);
  }

  function stopListening() {
    if (owner) abort();
  }

  function move(event: PointerEvent) {
    if (!holding.value) return;
    cancelling.value = Math.hypot(event.clientX - origin.x, event.clientY - origin.y) > CANCEL_SLIDE_PX;
  }

  function release(event: PointerEvent) {
    if (!holding.value || !take) return;
    (event.currentTarget as HTMLElement | null)?.releasePointerCapture?.(event.pointerId);
    if (event.type === 'pointercancel' || cancelling.value) return abort();
    settle();
  }

  function hold() {
    begin(false);
  }

  function letGo() {
    settle();
  }

  return { supported, holding, recognising, cancelling, transcript, start, move, release, hold, letGo, listen, stopListening };
}
