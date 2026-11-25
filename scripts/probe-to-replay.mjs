import { readFileSync, writeFileSync } from 'node:fs';
import { hostname, userInfo } from 'node:os';
import { parseArgs } from 'node:util';
import { claudeSteps, claudeWorkspace } from './probe-claude-steps.mjs';

const WORKSPACE = '<workspace>';
const ANY = '<any>';
const USAGE =
  'usage: node scripts/probe-to-replay.mjs <recording.jsonl> <out.ndjson> --provider <id> --scenario <name> [--lines 0-3,20-45] [--max-run 5] [--max-commands 5] [--drop <method regex>]\n       node scripts/probe-to-replay.mjs <out.ndjson> --provider claude --scenario <name> --part <claude-recording.jsonl>[:0-3,20-45] [--part ...]';

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    provider: { type: 'string' },
    scenario: { type: 'string' },
    lines: { type: 'string' },
    'max-run': { type: 'string', default: '5' },
    'max-commands': { type: 'string', default: '5' },
    drop: { type: 'string' },
    part: { type: 'string', multiple: true },
  },
});
const claude = (values.part ?? []).length > 0;
const [input, output] = claude ? [true, positionals[0]] : positionals;
if (!input || !output || !values.provider || !values.scenario) {
  console.error(USAGE);
  process.exit(1);
}

const frames = claude ? [] : readFileSync(input, 'utf8')
  .split('\n')
  .filter(Boolean)
  .map((line, index) => ({ index, ...JSON.parse(line) }))
  .filter((record) => record.dir === 'in' || record.dir === 'out')
  .map((record) => ({ index: record.index, dir: record.dir, message: JSON.parse(record.raw) }));

const parts = (values.part ?? []).map((part) => {
  const [file, lines = ''] = part.split(/:(?=[\d,-]*$)/);
  return { records: readFileSync(file, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line)), lines };
});
const cwd = claude
  ? claudeWorkspace(parts)
  : frames.find((f) => f.message.method === 'session/new')?.message.params.cwd;
const ranges = (values.lines ?? '')
  .split(',')
  .filter(Boolean)
  .map((range) => range.split('-').map(Number));
const selected = frames.filter(
  (f) => ranges.length === 0 || ranges.some(([from, to = from]) => f.index >= from && f.index <= to),
);

const SECRETS = [
  [/\b(?:sk|xai|ghp|gho|glpat|github_pat)[-_][A-Za-z0-9_-]{16,}/g, '<token>'],
  [/Bearer\s+[A-Za-z0-9._~+/-]{16,}/g, 'Bearer <token>'],
  [/eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{5,}/g, '<token>'],
  [/[\w.+-]+@[\w-]+\.[\w.-]+/g, '<email>'],
];
const user = userInfo().username;
const host = hostname();
const wordOf = (name) => new RegExp(`\\b${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'gi');
const scrubbed = new Map();
const note = (kind) => scrubbed.set(kind, (scrubbed.get(kind) ?? 0) + 1);
const replaceAll = (text, pattern, to, kind) =>
  text.replace(pattern, () => {
    note(kind);
    return to;
  });

const scrubText = (text) => {
  for (const variant of cwd ? [cwd, cwd.replaceAll('\\', '/')] : []) {
    text = replaceAll(text, new RegExp(variant.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g'), WORKSPACE, 'workspace');
  }
  text = text.replace(/<workspace>[\\/][^\s"'`)]*/g, (m) => m.replaceAll('\\', '/'));
  text = replaceAll(text, /[A-Za-z]:[\\/]Users[\\/][^\\/\s"']+/gi, '<home>', 'home');
  text = replaceAll(text, /\/(?:Users|home)\/[^/\s"']+/g, '<home>', 'home');
  text = replaceAll(text, wordOf(host), '<host>', 'host');
  text = replaceAll(text, wordOf(user), '<user>', 'user');
  for (const [pattern, to] of SECRETS) text = replaceAll(text, pattern, to, 'secret');
  return text;
};

const scrub = (value) => {
  if (typeof value === 'string') return scrubText(value);
  if (Array.isArray(value)) return value.map(scrub);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, scrub(v)]));
  return value;
};

const emit = (steps) => {
  const text = steps.map((step) => JSON.stringify(step)).join('\n') + '\n';
  const leaks = [user, host, 'C:\\\\Users', 'C:/Users'].filter((needle) => wordOf(needle).test(text));
  if (leaks.length > 0) {
    console.error(`refusing to write: ${leaks.join(', ')} still in the output`);
    process.exit(2);
  }
  writeFileSync(output, text);
  console.log(`${output}: ${steps.length} steps; scrubbed ${[...scrubbed].map(([k, n]) => `${k} x${n}`).join(', ') || 'nothing'}`);
};

if (claude) {
  emit(claudeSteps({ parts, scrub, scrubText, provider: values.provider, scenario: values.scenario }));
  process.exit(0);
}

const maxRun = Number(values['max-run']);
const maxCommands = Number(values['max-commands']);
const drop = values.drop ? new RegExp(values.drop) : null;
const kept = [];
let runKind = null;
let run = 0;
for (const frame of selected) {
  const { dir, message } = frame;
  if (dir === 'out' && message.method && message.id === undefined && drop?.test(message.method)) continue;
  const updated = dir === 'out' && message.method === 'session/update' ? message.params.update.sessionUpdate : null;
  const kind = updated === 'agent_thought_chunk' ? updated : null;
  run = kind !== null && kind === runKind ? run + 1 : 1;
  runKind = kind;
  if (kind !== null && run > maxRun) continue;
  kept.push(frame);
}

const agentRequests = new Map();
const clientIds = new Map();
const steps = [];
const initialized = frames.find((f) => f.dir === 'out' && f.message.result?.agentInfo);
steps.push({
  type: 'transcript_start',
  provider: values.provider,
  protocol: 'acp',
  version: initialized?.message.result.agentInfo.version ?? 'unknown',
  scenario: values.scenario,
});
steps.push({ type: 'berth', call: { call: 'create', args: { name: 'a' } } });

for (const { dir, message } of kept) {
  const body = scrub(structuredClone(message));
  const commands = body.params?.update?.availableCommands;
  if (commands) body.params.update.availableCommands = commands.slice(0, maxCommands);
  if (dir === 'out' && message.method) {
    if (message.id !== undefined) agentRequests.set(message.id, message);
    steps.push({ type: 'emit_inbound', label: message.method === 'session/update' ? `update:${message.params.update.sessionUpdate}` : message.method, frame: body });
  } else if (dir === 'out') {
    if (clientIds.has(message.id)) body.id = clientIds.get(message.id);
    steps.push({ type: 'emit_inbound', label: 'response', frame: body });
  } else if (message.method) {
    clientIds.set(message.id, clientIds.size + 1);
    body.id = clientIds.get(message.id);
    if (message.method === 'initialize') body.params.clientInfo = { name: 'berth', version: ANY };
    if (message.method === 'session/prompt') {
      const text = message.params.prompt.filter((b) => b.type === 'text').map((b) => b.text).join('\n');
      steps.push({ type: 'berth', call: { call: 'prompt', args: { text: scrubText(text) } } });
    }
    steps.push({ type: 'expect_outbound', label: message.method, frame: body });
  } else {
    if (message.error) body.error.message = ANY;
    const chosen = message.result?.outcome?.optionId;
    const options = agentRequests.get(message.id)?.params.options ?? [];
    if (chosen !== undefined) {
      steps.push({ type: 'berth', call: { call: 'answer', args: { option: options.findIndex((o) => o.optionId === chosen) } } });
    }
    steps.push({ type: 'expect_outbound', label: 'response', frame: body });
  }
}

emit(steps);
