import type { Checkpoint, FileEdit, TranscriptMessage } from '../../../entities/session';

export type ToolCategory = 'read' | 'edit' | 'run' | 'search' | 'fetch' | 'other';
export type ToolCall = { message: TranscriptMessage; category: ToolCategory; details: TranscriptMessage[] };
export type ChangedFile = FileEdit;
export type ChatBlock =
  | { type: 'prompt'; key: string; message: TranscriptMessage }
  | { type: 'turn'; key: string; seconds: number | null; model: string | null }
  | { type: 'tools'; key: string; calls: ToolCall[]; seconds: number | null }
  | { type: 'message'; key: string; message: TranscriptMessage }
  | { type: 'files'; key: string; files: ChangedFile[] };

const CATEGORY_BY_TOOL: Readonly<Record<string, ToolCategory>> = {
  Read: 'read',
  NotebookRead: 'read',
  read_file: 'read',
  view_image: 'read',
  Edit: 'edit',
  MultiEdit: 'edit',
  Write: 'edit',
  NotebookEdit: 'edit',
  apply_patch: 'edit',
  Bash: 'run',
  BashOutput: 'run',
  KillShell: 'run',
  shell: 'run',
  exec_command: 'run',
  write_stdin: 'run',
  local_shell: 'run',
  Grep: 'search',
  Glob: 'search',
  LS: 'search',
  WebSearch: 'search',
  web_search: 'search',
  WebFetch: 'fetch',
};

const CATEGORY_BY_KIND: Readonly<Record<string, ToolCategory>> = {
  read: 'read',
  edit: 'edit',
  delete: 'edit',
  move: 'edit',
  execute: 'run',
  search: 'search',
  fetch: 'fetch',
};

const TOOL_CATEGORIES: readonly ToolCategory[] = ['read', 'edit', 'run', 'search', 'fetch', 'other'];

const CHECKPOINT_SLACK_MS = 120_000;

function categoryOf(message: TranscriptMessage): ToolCategory {
  if (message.edits?.length) return 'edit';
  return CATEGORY_BY_TOOL[message.tool ?? ''] ?? CATEGORY_BY_KIND[message.toolKind ?? ''] ?? 'other';
}

const timeOf = (message: TranscriptMessage) => (message.at ? Date.parse(message.at) : Number.NaN);

function secondsBetween(first: TranscriptMessage | undefined, last: TranscriptMessage | undefined): number | null {
  if (!first || !last) return null;
  const span = timeOf(last) - timeOf(first);
  return Number.isFinite(span) && span >= 1000 ? Math.round(span / 1000) : null;
}

const isPrompt = (message: TranscriptMessage) => message.role === 'user' && message.tool === null && !message.shell;

function changedFiles(messages: readonly TranscriptMessage[]): ChangedFile[] {
  const byPath = new Map<string, ChangedFile>();
  for (const edit of messages.flatMap((message) => message.edits ?? [])) {
    const known = byPath.get(edit.path);
    byPath.set(edit.path, known ? { path: edit.path, additions: known.additions + edit.additions, deletions: known.deletions + edit.deletions } : { ...edit });
  }
  return [...byPath.values()];
}

function turnBlocks(prompt: TranscriptMessage | null, messages: TranscriptMessage[]): ChatBlock[] {
  const out: ChatBlock[] = [];
  if (prompt) out.push({ type: 'prompt', key: prompt.key, message: prompt });
  if (!messages.length) return out;
  const first = messages[0]!;
  const model = messages.findLast((message) => message.model)?.model ?? null;
  out.push({ type: 'turn', key: `${first.key}:turn`, seconds: secondsBetween(prompt ?? first, messages.at(-1)), model });
  let group: ToolCall[] | null = null;
  for (const message of messages) {
    if (message.callOf && group) {
      const call = group.findLast((entry) => entry.message.key === message.callOf);
      if (call) {
        call.details.push(message);
        continue;
      }
    }
    if (message.tool) {
      if (!group) {
        group = [];
        out.push({ type: 'tools', key: `${message.key}:tools`, calls: group, seconds: null });
      }
      group.push({ message, category: categoryOf(message), details: [] });
      continue;
    }
    group = null;
    out.push({ type: 'message', key: message.key, message });
  }
  for (const block of out) if (block.type === 'tools') block.seconds = secondsBetween(block.calls[0]?.message, block.calls.at(-1)?.message);
  const files = changedFiles(messages);
  if (files.length) out.push({ type: 'files', key: `${first.key}:files`, files });
  return out;
}

export function chatBlocks(messages: readonly TranscriptMessage[]): ChatBlock[] {
  const out: ChatBlock[] = [];
  let prompt: TranscriptMessage | null = null;
  let turn: TranscriptMessage[] = [];
  for (const message of messages) {
    if (isPrompt(message)) {
      out.push(...turnBlocks(prompt, turn));
      prompt = message;
      turn = [];
    } else turn.push(message);
  }
  out.push(...turnBlocks(prompt, turn));
  return out;
}

export function toolCounts(calls: readonly ToolCall[]): [ToolCategory, number][] {
  const counts = new Map<ToolCategory, number>();
  const edited = new Set<string>();
  for (const call of calls) {
    if (call.category === 'edit' && call.message.edits?.length) {
      for (const edit of call.message.edits) edited.add(edit.path);
      counts.set('edit', edited.size);
    } else counts.set(call.category, (counts.get(call.category) ?? 0) + 1);
  }
  return TOOL_CATEGORIES.filter((category) => counts.has(category)).map((category) => [category, counts.get(category)!]);
}

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
