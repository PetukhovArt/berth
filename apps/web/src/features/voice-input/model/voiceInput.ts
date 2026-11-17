import { computed, shallowRef, watch } from 'vue';
import { createDictation, type HoldToTalkIssue } from '../../../shared/lib/dictation';
import { useDock } from '../../../shared/model/dock';
import { useSettings } from '../../../shared/config/settings/store';
import { tr, type PlainKey } from '../../../shared/lib/i18n/translate';
import { describeError, enqueuePrompt, submitPrompt, useBerth, type SessionNode } from '../../../entities/session';

const NOTICE_MS = 3000;
const ARMED_MS = 10_000;
const ISSUES: Record<HoldToTalkIssue, PlainKey> = {
  denied: 'agents.sessionView.chatTalkDenied',
  short: 'voice.short',
  empty: 'agents.sessionView.chatTalkEmpty',
  failed: 'agents.sessionView.chatTalkFailed',
  offline: 'voice.issue.offline',
  loading: 'voice.issue.loading',
  phrase: 'voice.issue.phrase',
};

const { values } = useSettings();
const wakeKey = shallowRef<string | null>(null);
const armedAt = shallowRef(0);
const wakeHeard = shallowRef(false);
let pushKey: string | null = null;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

export const voiceNotice = shallowRef('');
export const wakePhrase = computed(() => values.value['voice.wakePhrase'].trim());

const sessionOf = (key: string | null): SessionNode | undefined => (key ? useBerth()?.sessions.value.get(key) : undefined);

export const pushSession = shallowRef<SessionNode | null>(null);
export const wakeSession = computed(() => sessionOf(wakeKey.value) ?? null);
export const wakeArmed = computed(() => armedAt.value > 0 || wakeHeard.value);

function notify(text: string) {
  voiceNotice.value = text;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => (voiceNotice.value = ''), NOTICE_MS);
}

async function deliver(key: string | null, text: string) {
  const session = sessionOf(key);
  if (!session) return notify(tr('voice.gone'));
  try {
    const ref = session.ref;
    const agent = 'Agent' in ref ? useBerth()?.entities.value.agents.get(ref.Agent) : undefined;
    if (agent?.kind === 'acp' && (agent.status === 'running' || agent.status === 'needs_feedback')) enqueuePrompt(agent.id, text);
    else await submitPrompt(ref, text);
    notify(tr('voice.sent', { name: session.name, text }));
  } catch (e) {
    notify(describeError(e));
  }
}

function word(text: string): string {
  return text.toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');
}

function afterPhrase(text: string, phrase: string): string | null {
  const wanted = phrase.split(/\s+/).map(word).filter(Boolean);
  const words = text.split(/\s+/).filter(Boolean);
  const keys = words.map(word);
  if (!wanted.length) return null;
  for (let i = 0; i + wanted.length <= keys.length; i++) {
    if (wanted.every((part, j) => keys[i + j] === part))
      return words
        .slice(i + wanted.length)
        .join(' ')
        .replace(/^[^\p{L}\p{N}]+/u, '');
  }
  return null;
}

const push = createDictation({
  onSend: (text) => deliver(pushKey, text),
  onIssue: (issue) => notify(tr(ISSUES[issue])),
  onSettled: resumeWake,
});

function listenFor(): string {
  if (armedAt.value && Date.now() - armedAt.value < ARMED_MS) return '';
  armedAt.value = 0;
  return wakePhrase.value;
}

const wake = createDictation({
  onSend: heard,
  wakePhrase: listenFor,
  onIssue: (issue) => {
    if (issue === 'short' || issue === 'empty') return;
    stopWakeWord();
    notify(tr(ISSUES[issue]));
  },
  onWoke: () => (wakeHeard.value = true),
  onSettled: () => {
    wakeHeard.value = false;
    resumeWake();
  },
});

export const pushHolding = push.holding;
export const pushRecognising = push.recognising;
export const voiceTranscript = computed(() => (push.holding.value ? push.transcript.value : wake.transcript.value));

function heard(text: string) {
  if (!wakeKey.value) return;
  if (armedAt.value && Date.now() - armedAt.value < ARMED_MS) {
    armedAt.value = 0;
    return deliver(wakeKey.value, text);
  }
  armedAt.value = 0;
  const rest = afterPhrase(text, wakePhrase.value);
  if (rest === null) return;
  if (rest) return deliver(wakeKey.value, rest);
  armedAt.value = Date.now();
}

function resumeWake() {
  if (wakeKey.value && !push.holding.value && !push.recognising.value && !wake.holding.value) wake.listen();
}

function sessionInView(): string | null {
  const key = useDock().activeKey.value;
  return sessionOf(key) ? key : null;
}

function release() {
  window.removeEventListener('keyup', release, true);
  window.removeEventListener('blur', release);
  push.letGo();
}

export function holdToTalk() {
  if (push.holding.value) return;
  if (!push.supported) return notify(tr('voice.unsupported'));
  const key = sessionInView();
  if (!key) return notify(tr('voice.noSession'));
  pushKey = key;
  pushSession.value = sessionOf(key) ?? null;
  wakeHeard.value = false;
  wake.stopListening();
  window.addEventListener('keyup', release, true);
  window.addEventListener('blur', release);
  push.hold();
}

export function stopWakeWord() {
  wakeKey.value = null;
  armedAt.value = 0;
  wakeHeard.value = false;
  wake.stopListening();
}

watch(wakeSession, (session) => {
  if (!wakeKey.value || session) return;
  stopWakeWord();
  notify(tr('voice.gone'));
});

export function toggleWakeWord() {
  if (wakeKey.value) return stopWakeWord();
  if (!wake.supported) return notify(tr('voice.unsupported'));
  const key = sessionInView();
  if (!key) return notify(tr('voice.noSession'));
  wakeKey.value = key;
  resumeWake();
}
