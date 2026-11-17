import type { IUnicodeVersionProvider, Terminal } from '@xterm/xterm';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { hostPlatform } from '../../../shared/lib/platform';

const VS16 = 0xfe0f;
const OWN_NARROW_CELL = 1 << 1;
const VS16_CELL_VERSION = '11-vs16-cell';

function unicode11(): IUnicodeVersionProvider {
  let provider: IUnicodeVersionProvider | undefined;
  const capture = { unicode: { register: (registered: IUnicodeVersionProvider) => (provider = registered) } };
  new Unicode11Addon().activate(capture as unknown as Terminal);
  if (!provider) throw new Error('Unicode11Addon registered no provider');
  return provider;
}

function vs16InOwnCell(base: IUnicodeVersionProvider): IUnicodeVersionProvider {
  return {
    version: VS16_CELL_VERSION,
    wcwidth: (codepoint) => (codepoint === VS16 ? 1 : base.wcwidth(codepoint)),
    charProperties: (codepoint, preceding) =>
      codepoint === VS16 ? OWN_NARROW_CELL : base.charProperties(codepoint, preceding),
  };
}

export function loadUnicodeWidths(term: Terminal): void {
  const base = unicode11();
  const provider = hostPlatform.value.capabilities.ptyGivesVs16ItsOwnCell ? vs16InOwnCell(base) : base;
  term.unicode.register(provider);
  term.unicode.activeVersion = provider.version;
}
