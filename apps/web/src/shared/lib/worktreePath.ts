const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

function rootPattern(root: string): RegExp | null {
  const parts = root.split(/[\\/]+/).filter(Boolean);
  if (!parts.length) return null;
  const lead = /^[\\/]/.test(root) ? '[\\\\/]' : '';
  return new RegExp(`${lead}${parts.map(escape).join('[\\\\/]+')}(?:[\\\\/]+|(?![^\\s"'\`]))`, 'gi');
}

export function withinWorktree(text: string, root: string | null | undefined): string {
  if (!root) return text;
  const pattern = rootPattern(root);
  if (!pattern) return text;
  return text.replace(pattern, (match) => (/[\\/]$/.test(match) ? '' : '.'));
}

export function worktreeRelative(path: string, root: string | null | undefined): string {
  return withinWorktree(path, root).replace(/\\/g, '/').replace(/^\.\//, '');
}
