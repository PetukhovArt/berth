import type { TranscriptMessage } from './external';
import { parseJigSteps } from './jigSteps';

type PlanState = 'done' | 'doing' | 'todo';
export type PlanItem = { text: string; state: PlanState };

type Input = Record<string, unknown> | null;

const STATES: Record<string, PlanState> = {
  completed: 'done',
  done: 'done',
  in_progress: 'doing',
  pending: 'todo',
};

const str = (value: unknown) => (typeof value === 'string' ? value.trim() : '');
const stateOf = (value: unknown): PlanState => STATES[str(value)] ?? 'todo';

function replaceAll(list: unknown, textKey: string): PlanItem[] | null {
  if (!Array.isArray(list)) return null;
  return list.flatMap((entry: Input) => {
    const text = str(entry?.[textKey]);
    return text ? [{ text, state: stateOf(entry?.status) }] : [];
  });
}

const JIG_STATES: Record<string, PlanState> = { active: 'doing', done: 'done' };

function applyJig(items: PlanItem[], command: string): PlanItem[] {
  let next = items;
  for (const step of parseJigSteps(command)) {
    if (step.kind === 'plan') {
      next = step.titles.map((text) => next.find((item) => item.text === text) ?? { text, state: 'todo' });
      continue;
    }
    const state = JIG_STATES[step.result.trim()] ?? 'todo';
    const exists = next.some((item) => item.text === step.title);
    next = exists ? next.map((item) => (item.text === step.title ? { text: item.text, state } : item)) : [...next, { text: step.title, state }];
  }
  return next;
}

export function derivePlan(messages: readonly TranscriptMessage[]): PlanItem[] {
  let items: PlanItem[] = [];
  let ids: string[] = [];
  for (const message of messages) {
    const input = message.input as Input | undefined;
    if (!message.tool || !input) continue;
    if (message.tool === 'Bash' || message.tool === 'shell' || message.tool === 'exec_command') {
      const command = str(input.command) || str(input.cmd);
      const next = command ? applyJig(items, command) : items;
      if (next !== items) {
        items = next;
        ids = next.map((_, index) => String(index + 1));
      }
      continue;
    }
    if (message.tool === 'TodoWrite' || message.tool === 'update_plan') {
      const next = replaceAll(message.tool === 'TodoWrite' ? input.todos : input.plan, message.tool === 'TodoWrite' ? 'content' : 'step');
      if (next) {
        items = next;
        ids = next.map((_, index) => String(index + 1));
      }
    } else if (message.tool === 'TaskCreate') {
      const text = str(input.subject);
      if (!text) continue;
      items = [...items, { text, state: 'todo' }];
      ids = [...ids, String(ids.length + 1)];
    } else if (message.tool === 'TaskUpdate') {
      const index = ids.indexOf(str(input.taskId));
      if (index < 0) continue;
      if (str(input.status) === 'deleted') {
        items = items.filter((_, at) => at !== index);
        ids = ids.filter((_, at) => at !== index);
        continue;
      }
      const current = items[index];
      if (!current) continue;
      items = items.map((item, at) => (at === index ? { text: str(input.subject) || item.text, state: input.status ? stateOf(input.status) : item.state } : item));
    }
  }
  return items;
}
