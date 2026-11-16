import { computed, shallowRef } from 'vue';
import { closeChannel, createChannel, invoke, isDesktop, listen } from '../../bridge';
import { hostPlatform } from './platform';
import type { DictationEngine, HoldToTalkIssue, Take, TakeEvents, TakeOptions } from './speechEngine';

type VoiceModel = { id: 'dictation' | 'wake'; ready: boolean; bytes: number; received: number | null; error: string | null };
type VoiceModels = { supported: boolean; dir: string | null; models: VoiceModel[] };

type RunEvent =
  | { kind: 'preparing' | 'listening' | 'woke' | 'speech' }
  | { kind: 'partial' | 'finished'; text: string }
  | { kind: 'failed'; issue: HoldToTalkIssue; message: string };

const MEGABYTE = 1024 * 1024;

const available = isDesktop && hostPlatform.value.os !== 'linux';

export const voiceModels = shallowRef<VoiceModels | null>(null);

export const voiceDownload = computed(() => {
  const loading = (voiceModels.value?.models ?? []).filter((model) => model.received !== null);
  if (!loading.length) return null;
  const received = loading.reduce((sum, model) => sum + (model.received ?? 0), 0);
  const total = loading.reduce((sum, model) => sum + model.bytes, 0);
  return { percent: Math.min(100, Math.floor((received / Math.max(total, 1)) * 100)), totalMb: Math.round(total / MEGABYTE) };
});

export function megabytes(bytes: number): number {
  return Math.round(bytes / MEGABYTE);
}

export async function removeVoiceModels(): Promise<void> {
  voiceModels.value = await invoke<VoiceModels>('voice_remove_models');
}

if (available) {
  void invoke<VoiceModels>('voice_models')
    .then((models) => (voiceModels.value = models))
    .catch(() => undefined);
  void listen<VoiceModels>('voice-models', (event) => (voiceModels.value = event.payload));
}

function open(options: TakeOptions, events: TakeEvents): Take {
  let run: number | null = null;
  let pending: 'voice_stop' | 'voice_cancel' | null = null;
  let woke = false;
  let ended = false;
  const end = (text: string, issue: HoldToTalkIssue | null) => {
    if (ended) return;
    ended = true;
    closeChannel(channel);
    events.end(text, issue);
  };
  const channel = createChannel<RunEvent>((event) => {
    if (event.kind === 'woke') {
      woke = true;
      events.woke();
    } else if (event.kind === 'speech') events.speech();
    else if (event.kind === 'partial') events.interim(event.text);
    else if (event.kind === 'finished') end(woke ? `${options.wakePhrase} ${event.text}`.trim() : event.text, null);
    else if (event.kind === 'failed') end('', event.issue);
  });
  const send = (command: 'voice_stop' | 'voice_cancel') => {
    if (run === null) pending = command;
    else void invoke(command, { run }).catch(() => undefined);
  };
  invoke<number>('voice_listen', { wakePhrase: options.wakePhrase || null, handsFree: options.handsFree, onEvent: channel })
    .then((id) => {
      run = id;
      if (pending) send(pending);
    })
    .catch(() => end('', 'failed'));
  return {
    stop: () => send('voice_stop'),
    abort: () => {
      ended = true;
      closeChannel(channel);
      send('voice_cancel');
    },
  };
}

export const nativeEngine: DictationEngine | null = available ? { supported: true, open } : null;
