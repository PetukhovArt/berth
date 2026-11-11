import { fsListMarkdown } from '../api/fs';

export type MarkdownEntry = { path: string; rel: string; name: string };

export const FILE_PANEL_PREFIX = 'file:';

export function normalizePath(path: string): string {
  return path.replace(/\\/g, '/');
}

export function filePanelId(path: string): string {
  return `${FILE_PANEL_PREFIX}${normalizePath(path)}`;
}

export function isFilePanel(id: string): boolean {
  return id.startsWith(FILE_PANEL_PREFIX);
}

export function fileNameOf(path: string): string {
  return normalizePath(path).split('/').pop() ?? path;
}

export async function listWorktreeMarkdown(root: string): Promise<MarkdownEntry[]> {
  const base = normalizePath(root).replace(/\/+$/, '');
  const paths = await fsListMarkdown(root);
  return paths.map((path) => {
    const rel = path.startsWith(`${base}/`) ? path.slice(base.length + 1) : path;
    return { path, rel, name: fileNameOf(path) };
  });
}
