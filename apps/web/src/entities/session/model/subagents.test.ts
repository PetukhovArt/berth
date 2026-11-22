import { describe, expect, it } from 'vitest';
import type { TranscriptMessage } from './external';
import type { SubagentFacts } from './shellTags';
import { deriveSubagents, stripVisible } from './subagents';

const T0 = Date.parse('2026-11-20T10:00:00Z');
const iso = (offsetSeconds: number) => new Date(T0 + offsetSeconds * 1000).toISOString();

function prompt(key: string, at: number): TranscriptMessage {
  return { key, role: 'user', text: key, tool: null, at: iso(at) };
}

function launch(id: string, at: number, report?: { status: string; waiting?: boolean; at: number }): TranscriptMessage {
  const facts: SubagentFacts = {
    call_id: id,
    description: `work ${id}`,
    kind: 'general-purpose',
    model: null,
    background: true,
    ...(report ? { report: { status: report.status, waiting: report.waiting ?? false, at: iso(report.at) } } : {}),
  };
  return { key: `${id}:0`, role: 'assistant', text: facts.description, tool: 'Agent', at: iso(at), subagent: facts };
}

describe('deriveSubagents', () => {
  const cases = [
    ['no report', undefined, 'running', 'stopped'],
    ['async launch ack', { status: 'async_launched', at: 1 }, 'running', 'stopped'],
    ['interim waiting notification', { status: 'completed', waiting: true, at: 5 }, 'running', 'stopped'],
    ['completed', { status: 'completed', at: 9 }, 'done', 'done'],
    ['failed', { status: 'failed', at: 9 }, 'failed', 'failed'],
    ['killed', { status: 'killed', at: 9 }, 'stopped', 'stopped'],
  ] as const;

  it.each(cases)('%s reads as %s while the session is live and %s once it is not', (_name, report, live, gone) => {
    const messages = [prompt('p1', 0), launch('a', 1, report)];
    expect(deriveSubagents(messages, true).all[0]?.state).toBe(live);
    expect(deriveSubagents(messages, false).all[0]?.state).toBe(gone);
  });

  it('keeps running subagents of earlier turns beside the current turn', () => {
    const messages = [
      prompt('p1', 0),
      launch('old-done', 1, { status: 'completed', at: 3 }),
      launch('old-running', 1),
      prompt('p2', 20),
      launch('new-done', 21, { status: 'completed', at: 25 }),
    ];
    expect(deriveSubagents(messages, true).current.map((agent) => agent.callId)).toEqual(['old-running', 'new-done']);
  });
});

describe('stripVisible', () => {
  const settled = deriveSubagents([prompt('p1', 0), launch('a', 1, { status: 'completed', at: 10 })], true).current;
  const mixed = deriveSubagents([prompt('p1', 0), launch('a', 1), launch('b', 1, { status: 'completed', at: 10 })], true).current;

  it('stays while one subagent runs, however long ago the others finished', () => {
    expect(stripVisible(mixed, T0 + 3_600_000)).toBe(true);
  });

  it('lingers after the last one finishes, then hides', () => {
    expect(stripVisible(settled, T0 + 15_000)).toBe(true);
    expect(stripVisible(settled, T0 + 31_000)).toBe(false);
  });
});
