import type { TranscriptMessage } from './external';
import type { SubagentFacts } from './shellTags';
import { isPrompt } from './transcriptLinks';

export type SubagentState = 'running' | 'done' | 'failed' | 'stopped';
type Launch = { callId: string; description: string; kind: string | null; model: string | null; background: boolean; startedAt: number | null; turn: number };
export type Subagent = Launch & ({ state: 'running' } | { state: Exclude<SubagentState, 'running'>; endedAt: number | null });
type SubagentRoster = { all: Subagent[]; current: Subagent[] };
type SubagentCounts = Record<SubagentState, number>;

const LINGER_MS = 20_000;

const SETTLED: Readonly<Record<string, Exclude<SubagentState, 'running'>>> = {
  completed: 'done',
  failed: 'failed',
  killed: 'stopped',
  stopped: 'stopped',
};

function instant(at: string | null | undefined): number | null {
  const ms = at ? Date.parse(at) : Number.NaN;
  return Number.isFinite(ms) ? ms : null;
}

function outcomeOf(facts: SubagentFacts, live: boolean): { state: 'running' } | { state: Exclude<SubagentState, 'running'>; endedAt: number | null } {
  const report = facts.report;
  const settled = report && !report.waiting ? SETTLED[report.status] : undefined;
  if (report && settled) return { state: settled, endedAt: instant(report.at) };
  return live ? { state: 'running' } : { state: 'stopped', endedAt: null };
}

export function deriveSubagents(messages: readonly TranscriptMessage[], live: boolean): SubagentRoster {
  const all: Subagent[] = [];
  let turn = 0;
  for (const message of messages) {
    if (isPrompt(message)) {
      turn += 1;
      continue;
    }
    const facts = message.subagent;
    if (!facts) continue;
    all.push({
      callId: facts.call_id,
      description: facts.description,
      kind: facts.kind,
      model: facts.model,
      background: facts.background,
      startedAt: instant(message.at),
      turn,
      ...outcomeOf(facts, live),
    });
  }
  return { all, current: all.filter((agent) => agent.state === 'running' || agent.turn === turn) };
}

function endedAt(agent: Subagent): number | null {
  return agent.state === 'running' ? null : agent.endedAt;
}

export function stripVisible(current: readonly Subagent[], now: number): boolean {
  return current.some((agent) => {
    if (agent.state === 'running') return true;
    const ended = endedAt(agent);
    return ended !== null && now - ended < LINGER_MS;
  });
}

export function subagentCounts(agents: readonly Subagent[]): SubagentCounts {
  const counts: SubagentCounts = { running: 0, done: 0, failed: 0, stopped: 0 };
  for (const agent of agents) counts[agent.state] += 1;
  return counts;
}

export function subagentSeconds(agent: Subagent, now: number): number | null {
  const end = agent.state === 'running' ? now : endedAt(agent);
  if (agent.startedAt === null || end === null) return null;
  return Math.max(0, Math.round((end - agent.startedAt) / 1000));
}
