import type { RepoRef } from './clones';
import type { Remote } from './remotes';

export function repoRefOf(item: { connector: string; project: string }, base: Remote | null | undefined): RepoRef {
  const path = [base?.path, item.project].filter(Boolean).join('/');
  const host = base?.host ?? item.connector;
  return { key: `${host}/${path.toLowerCase()}`, host, path };
}

export function projectOfRepo<P extends { repo_path: string }>(repo: RepoRef, projects: readonly P[], remotes: ReadonlyMap<string, Remote | null>): P | undefined {
  const full = repo.path.toLowerCase();
  return projects.find((candidate) => {
    const remote = remotes.get(candidate.repo_path);
    return remote?.host === repo.host && remote.path.toLowerCase() === full;
  });
}
