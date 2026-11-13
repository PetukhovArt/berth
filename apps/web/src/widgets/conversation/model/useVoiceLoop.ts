import { onBeforeUnmount, shallowRef } from 'vue';
import type { TranscriptMessage } from '../../../entities/session';

const SETTLE_MS = 2500;
const CHUNK_CHARS = 180;

type Phase = 'listen' | 'wait' | 'speak';

function speakable(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?(```|$)/g, ' ')
    .replace(/~~~[\s\S]*?(~~~|$)/g, ' ')
    .replace(/`[^`\n]*`/g, ' ')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, ' ')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/https?:\/\/\S+/g, ' ')
    .replace(/^\s*\|.*$/gm, ' ')
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+[.)])\s+/gm, '')
    .replace(/[*_~]+/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function chunks(text: string): string[] {
  const out: string[] = [];
  let current = '';
  for (const sentence of text.split(/(?<=[.!?…])\s+/)) {
    if (current && current.length + sentence.length + 1 > CHUNK_CHARS) {
      out.push(current);
      current = '';
    }
    current = current ? current + ' ' + sentence : sentence;
  }
  if (current) out.push(current);
  return out;
}

export function useVoiceLoop(options: { messages: () => TranscriptMessage[]; lang: () => string; listen: () => void; stop: () => void; listening: () => boolean }) {
  const supported = typeof window !== 'undefined' && 'speechSynthesis' in window;
  const active = shallowRef(false);
  const muted = shallowRef(false);
  const phase = shallowRef<Phase>('listen');
  let baseline = new Set<string>();
  let signature = '';
  let settleTimer: ReturnType<typeof setTimeout> | undefined;

  function snapshot(): string {
    const all = options.messages();
    const last = all.at(-1);
    return `${all.length}:${last?.key ?? ''}:${last?.text.length ?? 0}`;
  }

  function listenAgain() {
    if (!active.value || muted.value || options.listening()) return;
    options.listen();
  }

  function finishSpeaking() {
    if (!active.value || phase.value !== 'speak') return;
    phase.value = 'listen';
    listenAgain();
  }

  function speak(text: string) {
    const parts = chunks(text);
    if (!parts.length) {
      phase.value = 'listen';
      return listenAgain();
    }
    phase.value = 'speak';
    parts.forEach((part, index) => {
      const utterance = new SpeechSynthesisUtterance(part);
      utterance.lang = options.lang();
      if (index === parts.length - 1) {
        utterance.onend = finishSpeaking;
        utterance.onerror = finishSpeaking;
      }
      window.speechSynthesis.speak(utterance);
    });
    listenAgain();
  }

  function reply(): TranscriptMessage[] {
    return options.messages().filter((message) => !baseline.has(message.key) && message.role === 'assistant' && message.tool === null && message.text.trim());
  }

  function settle() {
    if (!active.value || phase.value !== 'wait') return;
    if (options.messages().at(-1)?.tool) return;
    const fresh = reply();
    if (!fresh.length) return;
    speak(fresh.map((message) => speakable(message.text)).join(' '));
  }

  function transcriptChanged() {
    if (!active.value || phase.value !== 'wait') return;
    const next = snapshot();
    if (next === signature) return;
    signature = next;
    clearTimeout(settleTimer);
    settleTimer = setTimeout(settle, SETTLE_MS);
  }

  function start() {
    if (active.value) return;
    active.value = true;
    muted.value = false;
    phase.value = 'listen';
    options.listen();
  }

  function end() {
    active.value = false;
    muted.value = false;
    phase.value = 'listen';
    clearTimeout(settleTimer);
    if (supported) window.speechSynthesis.cancel();
    options.stop();
  }

  function toggleMute() {
    muted.value = !muted.value;
    if (muted.value) options.stop();
    else if (phase.value !== 'wait') listenAgain();
  }

  function sent() {
    if (!active.value) return;
    phase.value = 'wait';
    baseline = new Set(options.messages().map((message) => message.key));
    signature = '';
    clearTimeout(settleTimer);
  }

  function unsent() {
    if (!active.value) return;
    phase.value = 'listen';
    listenAgain();
  }

  function onSpeech() {
    if (!active.value || phase.value !== 'speak') return;
    phase.value = 'listen';
    window.speechSynthesis.cancel();
  }

  function onSettled() {
    if (active.value && phase.value !== 'wait') listenAgain();
  }

  onBeforeUnmount(() => {
    clearTimeout(settleTimer);
    if (active.value && supported) window.speechSynthesis.cancel();
    active.value = false;
  });

  return { supported, active, muted, phase, start, end, toggleMute, sent, unsent, onSpeech, onSettled, transcriptChanged };
}
