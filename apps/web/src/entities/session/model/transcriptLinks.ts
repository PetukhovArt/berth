import type { Checkpoint } from './checkpoints';
import type { TranscriptMessage } from './external';

const CHECKPOINT_SLACK_MS = 120_000;

const timeOf = (message: TranscriptMessage) => (message.at ? Date.parse(message.at) : Number.NaN);

export const isPrompt = (message: TranscriptMessage) => message.role === 'user' && message.tool === null && !message.shell;

export function checkpointsByPrompt(messages: readonly TranscriptMessage[], checkpoints: readonly Checkpoint[]): Map<string, Checkpoint> {
  const prompts = messages.filter(isPrompt);
  const taken = checkpoints.filter((checkpoint) => !checkpoint.before_restore).sort((a, b) => a.created_at - b.created_at);
  const out = new Map<string, Checkpoint>();
  if (!prompts.length || !taken.length) return out;
  if (prompts.every((message) => Number.isNaN(timeOf(message)))) {
    const count = Math.min(prompts.length, taken.length);
    for (let i = 1; i <= count; i++) out.set(prompts[prompts.length - i]!.key, taken[taken.length - i]!);
    return out;
  }
  for (const checkpoint of taken) {
    const at = checkpoint.created_at * 1000;
    let best: TranscriptMessage | null = null;
    let bestGap = CHECKPOINT_SLACK_MS;
    for (const message of prompts) {
      const gap = Math.abs(timeOf(message) - at);
      if (gap <= bestGap) {
        best = message;
        bestGap = gap;
      }
    }
    if (best) out.set(best.key, checkpoint);
  }
  return out;
}

export function promptOfCheckpoint(messages: readonly TranscriptMessage[], checkpoints: readonly Checkpoint[]): Map<number, string> {
  const byKey = new Map(messages.map((message) => [message.key, message]));
  const out = new Map<number, string>();
  for (const [key, checkpoint] of checkpointsByPrompt(messages, checkpoints)) {
    const text = byKey.get(key)?.text.replace(/\s+/g, ' ').trim();
    if (text) out.set(checkpoint.number, text);
  }
  return out;
}

export function latestModel(messages: readonly TranscriptMessage[]): string | null {
  return messages.findLast((message) => message.model)?.model ?? null;
}

export function modelLabel(id: string): string {
  const trimmed = id.trim();
  const capital = (word: string) => word.charAt(0).toUpperCase() + word.slice(1);
  if (/^(opus|sonnet|haiku)$/i.test(trimmed)) return capital(trimmed.toLowerCase());
  const match = /^claude-(.+?)(?:-\d{8})?(\[1m\])?$/i.exec(trimmed);
  if (!match) return trimmed;
  const parts = match[1]!.split('-');
  const family = parts.find((part) => /^[a-z]+$/i.test(part));
  if (!family) return trimmed;
  const version = parts.filter((part) => /^\d+$/.test(part)).join('.');
  return [capital(family.toLowerCase()), version].filter(Boolean).join(' ') + (match[2] ? ' (1M)' : '');
}
