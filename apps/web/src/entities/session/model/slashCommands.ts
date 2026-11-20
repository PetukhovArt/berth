import { invoke } from '../../../bridge';
import { t } from '../../../shared/lib/i18n/i18n';
import { BerthCallError, call } from './berth';
import type { Harness } from './external';

export type SlashCommand = { name: string; description: string };
export type SlashSource = { harness: Harness; cwd: string } | { acpAgent: string };

const BUILT_INS = {
  claude: ['add-dir', 'agents', 'clear', 'compact', 'config', 'context', 'cost', 'doctor', 'export', 'help', 'hooks', 'init', 'mcp', 'memory', 'model', 'permissions', 'plugin', 'resume', 'review', 'rewind', 'security-review', 'status', 'usage'],
  codex: ['approvals', 'compact', 'diff', 'init', 'mcp', 'mention', 'model', 'new', 'review', 'status'],
} as const satisfies Record<Harness, readonly string[]>;

type BuiltIn = (typeof BUILT_INS)[Harness][number];

const BUILT_IN_KEY = {
  'add-dir': 'addDir',
  agents: 'agents',
  approvals: 'approvals',
  clear: 'clear',
  compact: 'compact',
  config: 'config',
  context: 'context',
  cost: 'cost',
  diff: 'diff',
  doctor: 'doctor',
  export: 'export',
  help: 'help',
  hooks: 'hooks',
  init: 'init',
  mcp: 'mcp',
  memory: 'memory',
  mention: 'mention',
  model: 'model',
  new: 'new',
  permissions: 'permissions',
  plugin: 'plugin',
  resume: 'resume',
  review: 'review',
  rewind: 'rewind',
  'security-review': 'securityReview',
  status: 'status',
  usage: 'usage',
} as const satisfies Record<BuiltIn, string>;

function builtIns(harness: Harness): SlashCommand[] {
  return BUILT_INS[harness].map((name) => ({ name, description: t(`chat.slash.builtIn.${BUILT_IN_KEY[name]}`) }));
}

async function acpCommands(agent: string): Promise<SlashCommand[]> {
  const reply = await call({ AcpCommands: { agent } });
  if (!('AcpCommands' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.AcpCommands.commands;
}

export async function listSlashCommands(source: SlashSource): Promise<SlashCommand[]> {
  if ('acpAgent' in source) return acpCommands(source.acpAgent);
  const found = await invoke<SlashCommand[]>('slash_commands', { harness: source.harness, cwd: source.cwd }).catch(() => []);
  const seen = new Set<string>();
  return [...builtIns(source.harness), ...found].filter((command) => {
    const key = command.name.toLowerCase();
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function matchSlashCommands(commands: readonly SlashCommand[], query: string): SlashCommand[] {
  const needle = query.toLowerCase();
  if (!needle) return [...commands].sort((a, b) => a.name.localeCompare(b.name));
  const rank = (name: string) => {
    const lower = name.toLowerCase();
    if (lower.startsWith(needle)) return 0;
    if (lower.split(':').some((part) => part.startsWith(needle))) return 1;
    if (lower.includes(needle)) return 2;
    return 3;
  };
  return commands
    .map((command) => ({ command, rank: rank(command.name) }))
    .filter((entry) => entry.rank < 3)
    .sort((a, b) => a.rank - b.rank || a.command.name.length - b.command.name.length || a.command.name.localeCompare(b.command.name))
    .map((entry) => entry.command);
}
