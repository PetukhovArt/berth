import type { AcpItem, AcpItemAt, AcpPlanEntry } from './berth';
import type { TranscriptMessage } from './external';
import type { PlanItem } from './plan';
import { t } from '../../../shared/lib/i18n/i18n';

const OUTPUT_CHARS = 4000;

const PLAN_STATES: Record<string, PlanItem['state']> = { completed: 'done', in_progress: 'doing', pending: 'todo' };
const EXIT_BY_STATUS: Readonly<Record<string, number>> = { completed: 0, failed: 1 };
const TOOL_STATUSES =['pending', 'in_progress', 'failed'] as const;

const isKnownStatus = (status: string): status is (typeof TOOL_STATUSES)[number] => (TOOL_STATUSES as readonly string[]).includes(status);
const toolStatus = (status: string) => (isKnownStatus(status) ? t(`agents.sessionView.toolStatus.${status}`) : status);

const fenced = (text: string, lang: string) => {
  const clipped = text.length > OUTPUT_CHARS ? `${text.slice(0, OUTPUT_CHARS)}\n…` : text;
  const fence = clipped.includes('```') ? '````' : '```';
  return `${fence}${lang}\n${clipped}\n${fence}`;
};

const quoted = (text: string) =>
  text
    .split('\n')
    .map((line) => `> ${line}`)
    .join('\n');

function diffText(diff: { path: string; old: string | null; new: string }): string {
  const removed = (diff.old ?? '').split('\n').filter((line, index, lines) => line || index < lines.length - 1);
  const added = diff.new.split('\n');
  return [`--- ${diff.path}`, `+++ ${diff.path}`, ...removed.map((line) => `-${line}`), ...added.map((line) => `+${line}`)].join('\n');
}

const splitLines = (text: string) => (text ? text.replace(/\n$/, '').split('\n') : []);

function lineDelta(before: string, after: string): { additions: number; deletions: number } {
  const old = splitLines(before);
  const next = splitLines(after);
  let head = 0;
  while (head < old.length && head < next.length && old[head] === next[head]) head++;
  let tail = 0;
  while (tail < old.length - head && tail < next.length - head && old[old.length - 1 - tail] === next[next.length - 1 - tail]) tail++;
  return { additions: next.length - head - tail, deletions: old.length - head - tail };
}

function toMessages(seq: number, item: AcpItem): TranscriptMessage[] {
  const key = `acp:${seq}`;
  const base = { key, at: null, tool: null };
  if ('User' in item) return [{ ...base, role: 'user', text: item.User.text, ...(item.User.attachments?.length ? { attachments: item.User.attachments } : {}) }];
  if ('Assistant' in item) return [{ ...base, role: 'assistant', text: item.Assistant.text }];
  if ('Thought' in item) return [{ ...base, role: 'assistant', text: quoted(item.Thought.text) }];
  if ('Notice' in item) return [{ ...base, role: 'assistant', text: `_${item.Notice.text}_` }];
  if ('Plan' in item) return [];
  const tool = item.Tool;
  const label = [tool.detail, tool.status === 'completed' ? '' : toolStatus(tool.status)].filter(Boolean).join(' · ');
  const edits = tool.diff ? [{ path: tool.diff.path, ...lineDelta(tool.diff.old ?? '', tool.diff.new) }] : [];
  const exit = tool.kind === 'execute' ? EXIT_BY_STATUS[tool.status] : undefined;
  const out: TranscriptMessage[] = [{ ...base, role: 'assistant', tool: tool.title || tool.kind, text: label, toolKind: tool.kind, edits, ...(exit === undefined ? {} : { exit }) }];
  if (tool.diff) out.push({ ...base, key: `${key}:diff`, role: 'assistant', text: tool.diff.path, shell: 'output', markdown: fenced(diffText(tool.diff), 'diff'), callOf: key });
  if (tool.output) out.push({ ...base, key: `${key}:output`, role: 'assistant', text: tool.output, shell: 'output', markdown: fenced(tool.output, ''), callOf: key });
  return out;
}

const converted = new WeakMap<AcpItem, TranscriptMessage[]>();

function messagesOf(at: AcpItemAt): TranscriptMessage[] {
  let hit = converted.get(at.item);
  if (!hit) {
    hit = toMessages(at.seq, at.item);
    converted.set(at.item, hit);
  }
  return hit;
}

export type RewindOption = { seq: number; files: boolean };

export function acpRewinds(items: readonly AcpItemAt[]): Map<string, RewindOption> {
  const out = new Map<string, RewindOption>();
  let previousAfter = true;
  for (const { seq, item } of items) {
    if (!('User' in item)) continue;
    const rewind = item.User.rewind;
    if (rewind && previousAfter) out.set(`acp:${seq}`, { seq, files: rewind.checkpoint !== null });
    previousAfter = !!rewind?.after;
  }
  return out;
}

export const acpMessages = (items: readonly AcpItemAt[]): TranscriptMessage[] => items.flatMap(messagesOf);

const toPlan = (entries: AcpPlanEntry[]): PlanItem[] => entries.map((entry) => ({ text: entry.content, state: PLAN_STATES[entry.status] ?? 'todo' }));

export function acpPlan(items: readonly AcpItemAt[]): PlanItem[] {
  const last = items.findLast((at) => 'Plan' in at.item)?.item;
  return last && 'Plan' in last ? toPlan(last.Plan.entries) : [];
}
