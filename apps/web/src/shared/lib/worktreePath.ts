const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const TOKEN = '[^\\s"\'`]';

function rootPattern(root: string): RegExp | null {
  const parts = root.split(/[\\/]+/).filter(Boolean);
  if (!parts.length) return null;
  const lead = /^[\\/]/.test(root) ? '[\\\\/]' : '';
  return new RegExp(`${lead}${parts.map(escape).join('[\\\\/]+')}(?:[\\\\/]+(${TOKEN}*)|(?!${TOKEN}))`, 'gi');
}

const forward = (text: string) => text.replace(/\\/g, '/');

export function withinWorktree(text: string, root: string | null | undefined): string {
  const pattern = root ? rootPattern(root) : null;
  const inside = pattern ? text.replace(pattern, (_match, tail: string | undefined) => (tail === undefined ? '.' : forward(tail))) : text;
  return new RegExp(`^${TOKEN}+$`).test(inside) ? forward(inside) : inside;
}

export function worktreeRelative(path: string, root: string | null | undefined): string {
  return forward(withinWorktree(path, root)).replace(/^\.\//, '');
}
