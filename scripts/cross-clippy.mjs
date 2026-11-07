#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const TARGETS = {
  macos: { triple: 'aarch64-apple-darwin', packages: ['berth-core', 'berth-daemon', 'berth-tui', 'berth', 'berth-desktop'] },
  linux: { triple: 'x86_64-unknown-linux-gnu', packages: ['berth-core', 'berth-daemon', 'berth-tui', 'berth'] },
};

const wanted = process.argv.slice(2);
const picked = wanted.length ? wanted : Object.keys(TARGETS);
const windows = process.platform === 'win32';

const stubDir = join(tmpdir(), 'berth-cross-clippy');
mkdirSync(stubDir, { recursive: true });
const stubJs = join(stubDir, 'stub.mjs');
writeFileSync(
  stubJs,
  [
    "import { writeFileSync } from 'node:fs';",
    'const args = process.argv.slice(2);',
    "const out = args.indexOf('-o');",
    "if (out >= 0 && args[out + 1]) writeFileSync(args[out + 1], '');",
    "const archive = args.find((arg) => arg.endsWith('.a'));",
    "if (process.env.BERTH_STUB_AR && archive) writeFileSync(archive, '!<arch>\\n');",
    '',
  ].join('\n'),
);

function stub(name, archiver) {
  const path = join(stubDir, windows ? `${name}.cmd` : name);
  const flag = archiver ? (windows ? 'set BERTH_STUB_AR=1&& ' : 'BERTH_STUB_AR=1 ') : '';
  const body = windows ? `@${flag}node "${stubJs}" %*\r\n` : `#!/bin/sh\n${flag}exec node "${stubJs}" "$@"\n`;
  writeFileSync(path, body, { mode: 0o755 });
  return path;
}

const cc = stub('stub-cc', false);
const ar = stub('stub-ar', true);

let failed = false;
for (const name of picked) {
  const target = TARGETS[name];
  if (!target) {
    console.error(`unknown target ${name}; pick from ${Object.keys(TARGETS).join(', ')}`);
    process.exit(2);
  }
  const key = target.triple.replaceAll('-', '_');
  const env = { ...process.env, [`CC_${key}`]: cc, [`AR_${key}`]: ar, CARGO_TARGET_DIR: join('target', 'cross') };
  const args = ['clippy', ...target.packages.flatMap((p) => ['-p', p]), '--all-targets', '--target', target.triple, '--', '-D', 'warnings'];
  console.log(`== ${name}: cargo ${args.join(' ')}`);
  const result = spawnSync('cargo', args, { stdio: 'inherit', env });
  if (result.status !== 0) failed = true;
}
process.exit(failed ? 1 : 0);
