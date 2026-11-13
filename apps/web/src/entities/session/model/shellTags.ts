type ShellKind = 'command' | 'output';
export type ShellMessage = { key: string; role: 'user' | 'assistant'; text: string; tool: string | null; at: string | null; input?: unknown; shell?: ShellKind; markdown?: string };

const TAGS = ['bash-input', 'bash-stdout', 'bash-stderr', 'local-command-stdout', 'local-command-stderr', 'local-command-caveat', 'command-name', 'command-message', 'command-args'] as const;
const WRAPPER = new RegExp(`<(${TAGS.join('|')})>([\\s\\S]*?)</\\1>`, 'g');
const ANSI = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*[A-Za-z]`, 'g');

const clean = (text: string) => text.replace(ANSI, '').trim();

function parse(text: string): { rest: string; fields: Map<string, string> } {
  const fields = new Map<string, string>();
  const rest = text.replace(WRAPPER, (_, tag: string, body: string) => {
    fields.set(tag, [fields.get(tag), clean(body)].filter(Boolean).join('\n'));
    return '';
  });
  return { rest: rest.trim(), fields };
}

export function withShellBlocks<T extends ShellMessage>(messages: T[]): T[] {
  const out: T[] = [];
  for (const message of messages) {
    if (message.role !== 'user' || message.tool !== null || !message.text.includes('<')) {
      out.push(message);
      continue;
    }
    const { rest, fields } = parse(message.text);
    if (!fields.size) {
      out.push(message);
      continue;
    }
    if (rest) {
      out.push({ ...message, text: rest });
      continue;
    }
    const command = fields.get('bash-input') ?? [fields.get('command-name'), fields.get('command-args')].filter(Boolean).join(' ');
    if (command) {
      out.push({ ...message, text: command, shell: 'command', markdown: fenced(command.startsWith('/') ? command : `$ ${command}`, 'sh') });
      continue;
    }
    const output = ['bash-stdout', 'local-command-stdout', 'bash-stderr', 'local-command-stderr']
      .map((tag) => fields.get(tag))
      .filter(Boolean)
      .join('\n');
    if (output) out.push({ ...message, text: output, shell: 'output', markdown: fenced(output) });
  }
  return out;
}

function fenced(text: string, language = ''): string {
  const longest = Math.max(2, ...[...text.matchAll(/`+/g)].map((run) => run[0].length));
  const fence = '`'.repeat(longest + 1);
  return `${fence}${language}\n${text}\n${fence}`;
}
