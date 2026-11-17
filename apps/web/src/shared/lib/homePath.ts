const HOME_PREFIX = /^(?:[A-Za-z]:)?[\\/](?:Users|home)[\\/][^\\/]+(?=[\\/]|$)/;

export const forwardSlashes = (path: string) => path.replace(/\\/g, '/');

export function withTilde(path: string): string {
  const match = HOME_PREFIX.exec(path);
  if (!match) return path;
  const rest = path.slice(match[0].length).replace(/\\/g, '/');
  return rest ? `~${rest}` : '~';
}
