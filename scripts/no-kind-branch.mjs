import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const CAP = 0;

const ROOTS = ['crates', 'apps/desktop/src-tauri/src'];
const SKIPPED_PATH = /(^|\/)(tests|berth-transcripts|driver)\/|(^|\/)(tests|[a-z_]+_tests)\.rs$|\/acp\/driver\/|^crates\/berth-core\/src\/agent\.rs$|^crates\/berth-daemon\/src\/agent\//;
const KIND_VARIANT = /\bAgentKind::(Claude|Codex|Cursor|OpenCode|Grok|Acp)\b/;
const KIND_LITERAL = /"(claude|codex|cursor|opencode|grok)"/;
const OTHER_KIND = /\b(?!AgentKind\b)\w+Kind::/;
const BRANCHING = /=>|==|!=|matches!/;

const root = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '--', ...ROOTS.map((r) => `${r}/*.rs`)], {
  cwd: root,
  encoding: 'utf8',
})
  .split('\n')
  .filter(Boolean);

const braces = (line) => {
  const code = line.replace(/\br(#*)"[^]*?"\1/g, '""').replace(/"(\\.|[^"\\])*"/g, '""').replace(/'(\\.|[^'\\])'/g, "''").replace(/\/\/.*$/, '');
  return (code.match(/{/g)?.length ?? 0) - (code.match(/}/g)?.length ?? 0);
};

const sources = new Map(files.map((file) => [file, readFileSync(join(root, file), 'utf8').split(/\r?\n/)]));

const testOnlyFiles = new Set();
for (const [file, lines] of sources) {
  lines.forEach((line, i) => {
    const declared = lines[i + 1]?.match(/^\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+(\w+);/);
    if (/^\s*#\[cfg\(test\)\]/.test(line) && declared) {
      testOnlyFiles.add(join(dirname(file), `${declared[1]}.rs`).replaceAll('\\', '/'));
      testOnlyFiles.add(join(dirname(file), declared[1], 'mod.rs').replaceAll('\\', '/'));
    }
  });
}

const hits = [];
for (const [file, lines] of sources) {
  if (SKIPPED_PATH.test(file) || testOnlyFiles.has(file)) continue;
  let depth = 0;
  let skipping = false;
  let armed = false;
  let inMigrations = false;
  lines.forEach((line, i) => {
    if (/^\s*#\[cfg\(test\)\]/.test(line)) armed = true;
    if (/\bconst MIGRATIONS\b/.test(line)) inMigrations = true;
    const skipLine = skipping || armed || inMigrations;
    if (armed && /[{;]\s*$/.test(line) && !/^\s*#\[/.test(line)) {
      armed = false;
      if (/{\s*$/.test(line)) {
        skipping = true;
        depth = 0;
      }
    }
    if (skipping) {
      depth += braces(line);
      if (depth <= 0) skipping = false;
    }
    if (inMigrations && /^\];/.test(line)) inMigrations = false;
    if (skipLine || /^\s*\/\//.test(line)) return;
    if (KIND_VARIANT.test(line) || (KIND_LITERAL.test(line) && BRANCHING.test(line) && !OTHER_KIND.test(line))) hits.push(`${file}:${i + 1}: ${line.trim()}`);
  });
}

const count = hits.length;
if (process.argv.includes('--list')) console.log(hits.join('\n'));
console.log(`agent kind branches outside the adapter: ${count} (cap ${CAP})`);
if (count > CAP) {
  console.error(`over the cap by ${count - CAP}: move the branch into an agent driver, or lower the cap with the code that removed it`);
  process.exit(1);
}
