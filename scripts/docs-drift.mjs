import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const codeOf = (markdown) => {
  const parts = [];
  let fenced = false;
  for (const line of markdown.split(/\r?\n/)) {
    if (/^\s*```/.test(line)) {
      fenced = !fenced;
      continue;
    }
    if (fenced) parts.push(line.replace(/(^|\s)#.*$/, ''));
    else for (const match of line.matchAll(/`([^`]+)`/g)) parts.push(match[1]);
  }
  return parts;
};

export const mentionedCommands = (markdown) =>
  codeOf(markdown).flatMap((code) =>
    [...code.matchAll(/(?:^|[\s"'(])berth((?: [a-z][a-z0-9-]*){1,2})/g)].map((match) => match[1].trim().split(' ')),
  );

export const documentedKeys = (markdown) => {
  const listed = new Set();
  const internal = new Set();
  let inKeyTable = false;
  for (const line of markdown.split(/\r?\n/)) {
    if (/^\|\s*Key\s*\|/.test(line)) {
      inKeyTable = true;
      continue;
    }
    if (!line.startsWith('|')) inKeyTable = false;
    if (inKeyTable && !/^\|\s*-/.test(line)) {
      const first = line.split('|')[1] ?? '';
      for (const match of first.matchAll(/`([^`]+)`/g)) listed.add(match[1]);
    }
    if (line.startsWith('Internal keys')) for (const match of line.matchAll(/`([^`]+)`/g)) internal.add(match[1]);
  }
  return { listed, internal };
};

export function drift({ commandPaths, codeKeys, cliDoc, configDoc }) {
  const paths = new Set(commandPaths);
  const mentionsInCli = mentionedCommands(cliDoc);
  const startsWith = (words, path) => path.split(' ').every((word, index) => words[index] === word);
  const missingCommands = [...paths].filter((path) => !mentionsInCli.some((words) => startsWith(words, path)));
  const hasChildren = (word) => [...paths].some((path) => path.startsWith(`${word} `));
  const extraCommands = new Set();
  for (const words of [...mentionsInCli, ...mentionedCommands(configDoc)]) {
    if (!paths.has(words[0])) extraCommands.add(words[0]);
    else if (words[1] && hasChildren(words[0]) && !paths.has(`${words[0]} ${words[1]}`)) extraCommands.add(`${words[0]} ${words[1]}`);
  }
  const { listed, internal } = documentedKeys(configDoc);
  const documented = new Set([...listed, ...internal]);
  const keys = new Set(codeKeys);
  return {
    missingCommands,
    extraCommands: [...extraCommands],
    missingKeys: [...keys].filter((key) => !documented.has(key)),
    extraKeys: [...documented].filter((key) => !keys.has(key)),
  };
}

const helpCommands = (binary, args) => {
  const text = execFileSync(binary, ['help', ...args], { encoding: 'utf8' });
  const section = text.split(/\r?\n/);
  const start = section.findIndex((line) => line.trim() === 'Commands:');
  if (start < 0) return [];
  const names = [];
  for (const line of section.slice(start + 1)) {
    if (line.trim() === '') break;
    const match = /^ {2}([a-z][a-z0-9-]*)(?:\s|$)/.exec(line);
    if (match && match[1] !== 'help') names.push(match[1]);
  }
  return names;
};

export const binaryCommandPaths = (binary) =>
  helpCommands(binary, []).flatMap((name) => [name, ...helpCommands(binary, [name]).map((child) => `${name} ${child}`)]);

const between = (text, start, end) => {
  const from = text.indexOf(start);
  if (from < 0) throw new Error(`${start} not found`);
  const to = text.indexOf(end, from);
  return text.slice(from, to < 0 ? undefined : to);
};

export const settingsKeys = (root) => {
  const schema = between(readFileSync(join(root, 'apps/web/src/shared/config/settings/schema.ts'), 'utf8'), 'export const SETTINGS = {', '} as const satisfies');
  const desktop = [...schema.matchAll(/^ {2}'?([A-Za-z_][A-Za-z0-9_.]*)'?: \{/gm)].map((match) => match[1]);
  const config = between(readFileSync(join(root, 'crates/berth-daemon/src/config.rs'), 'utf8'), 'pub struct Config {', '\n}');
  const daemon = [...config.matchAll(/^\s+pub (\w+):/gm)].map((match) => match[1]);
  const defaults = Object.keys(JSON.parse(readFileSync(join(root, 'crates/berth-core/daemon-defaults.json'), 'utf8')));
  return [...new Set([...desktop, ...daemon, ...defaults])];
};

const main = () => {
  const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const binary = process.argv[2] ?? process.env.BERTH_BIN ?? join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', `berth${suffix}`);
  if (!existsSync(binary)) {
    console.error(`docs-drift: no berth binary at ${binary}; build it or pass its path`);
    process.exit(2);
  }
  const result = drift({
    commandPaths: binaryCommandPaths(binary),
    codeKeys: settingsKeys(root),
    cliDoc: readFileSync(join(root, 'docs/cli.md'), 'utf8'),
    configDoc: readFileSync(join(root, 'docs/configuration.md'), 'utf8'),
  });
  const report = [
    ['commands the binary has that docs/cli.md does not show', result.missingCommands.map((path) => `berth ${path}`)],
    ['commands the docs name that the binary does not have', result.extraCommands.map((path) => `berth ${path}`)],
    ['settings keys missing from the key tables or the internal list in docs/configuration.md', result.missingKeys],
    ['keys docs/configuration.md lists that the code does not have', result.extraKeys],
  ].filter(([, items]) => items.length > 0);
  if (report.length === 0) {
    console.log('docs-drift: docs/cli.md and docs/configuration.md match the code');
    return;
  }
  for (const [title, items] of report) console.error(`docs-drift: ${title}:\n${items.map((item) => `  ${item}`).join('\n')}`);
  process.exit(1);
};

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
