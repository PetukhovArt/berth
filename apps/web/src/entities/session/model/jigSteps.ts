export type JigStep = { kind: 'plan'; titles: string[] } | { kind: 'step'; title: string; result: string };

const SEPARATORS = new Set(['&&', '||', ';', '|', '&']);
const ESCAPABLE = new Set(['"', '\\', '$', '`']);

function tokenize(command: string): string[] {
  const tokens: string[] = [];
  let current = '';
  let started = false;
  let quote: '"' | "'" | null = null;
  const flush = () => {
    if (started) tokens.push(current);
    current = '';
    started = false;
  };
  for (let i = 0; i < command.length; i++) {
    const ch = command[i]!;
    if (quote === "'") {
      if (ch === "'") quote = null;
      else current += ch;
    } else if (quote === '"') {
      if (ch === '"') quote = null;
      else if (ch === '\\' && ESCAPABLE.has(command[i + 1] ?? '')) current += command[++i];
      else current += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      started = true;
    } else if (ch === '\\' && i + 1 < command.length) {
      current += command[++i];
      started = true;
    } else if (/\s/.test(ch)) {
      flush();
    } else if (ch === ';' || ch === '&' || ch === '|') {
      flush();
      const pair = command[i + 1] === ch && ch !== ';' ? ch + ch : ch;
      i += pair.length - 1;
      tokens.push(pair);
    } else {
      current += ch;
      started = true;
    }
  }
  flush();
  return tokens;
}

function resultOf(args: string[]): string {
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    if (arg === '--result') return args[i + 1] ?? '';
    if (arg.startsWith('--result=')) return arg.slice('--result='.length);
  }
  return '';
}

export function parseJigSteps(command: string): JigStep[] {
  if (!command.includes('task-status')) return [];
  const tokens = tokenize(command);
  const steps: JigStep[] = [];
  for (let at = tokens.indexOf('task-status'); at >= 0; at = tokens.indexOf('task-status', at + 1)) {
    let end = at + 1;
    while (end < tokens.length && !SEPARATORS.has(tokens[end]!)) end++;
    const [verb, ...rest] = tokens.slice(at + 1, end);
    if (verb === 'plan') {
      const titles = rest.map((title) => title.trim()).filter((title) => title && !title.startsWith('--'));
      if (titles.length) steps.push({ kind: 'plan', titles });
    } else if (verb === 'log' && rest[0] === 'step') {
      const title = rest.slice(1).find((arg) => !arg.startsWith('--'))?.trim();
      if (title) steps.push({ kind: 'step', title, result: resultOf(rest) });
    }
  }
  return steps;
}
