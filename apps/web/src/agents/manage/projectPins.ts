import type { Entities } from '../berth';

type Projects = Entities['projects'];

function pinnedIds(projects: Projects): string[] {
  return [...projects.values()]
    .filter((project) => project.pin_order !== null)
    .sort((a, b) => (a.pin_order ?? 0) - (b.pin_order ?? 0))
    .map((project) => project.id);
}

export function pinnedFirst<T extends { id: string }>(items: readonly T[], projects: Projects | undefined): T[] {
  const rank = (item: T) => projects?.get(item.id)?.pin_order ?? null;
  const pinned = items.filter((item) => rank(item) !== null).sort((a, b) => (rank(a) ?? 0) - (rank(b) ?? 0));
  return [...pinned, ...items.filter((item) => rank(item) === null)];
}

type PinMove = -1 | 1;

function neighbour(projects: Projects, id: string, step: PinMove): string | undefined {
  const workspace = projects.get(id)?.workspace_id;
  const siblings = pinnedIds(projects).filter((other) => projects.get(other)?.workspace_id === workspace);
  const index = siblings.indexOf(id);
  return index < 0 ? undefined : siblings[index + step];
}

export function canMove(projects: Projects, id: string, step: PinMove): boolean {
  return neighbour(projects, id, step) !== undefined;
}

export function pinned(projects: Projects, id: string): string[] {
  return [...pinnedIds(projects).filter((other) => other !== id), id];
}

export function unpinned(projects: Projects, id: string): string[] {
  return pinnedIds(projects).filter((other) => other !== id);
}

export function moved(projects: Projects, id: string, step: PinMove): string[] {
  const order = pinnedIds(projects);
  const other = neighbour(projects, id, step);
  if (other === undefined) return order;
  const from = order.indexOf(id);
  const to = order.indexOf(other);
  order[from] = other;
  order[to] = id;
  return order;
}
