import type { DiffFile } from '../../../shared/api/connectors/api';

export type TreePlace = { dirs: string[]; name: string };

const isMoved = (file: DiffFile) => file.status === 'renamed' && file.old_path !== file.new_path;

export function treePlace(file: DiffFile): TreePlace {
  const to = file.new_path.split('/');
  if (!isMoved(file)) return { dirs: to.slice(0, -1), name: to.at(-1)! };
  const from = file.old_path.split('/');
  const limit = Math.min(from.length, to.length) - 1;
  let shared = 0;
  while (shared < limit && from[shared] === to[shared]) shared++;
  if (shared === 0) return { dirs: from.slice(0, -1), name: `${from.at(-1)!} → ${to.join('/')}` };
  return { dirs: to.slice(0, shared), name: `${from.slice(shared).join('/')} → ${to.slice(shared).join('/')}` };
}

export function flatName(file: DiffFile): string {
  return isMoved(file) ? `${file.old_path} → ${file.new_path}` : file.new_path;
}
