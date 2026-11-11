import { useDock, type PanelSpec } from '../../../shared/model/dock';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { itemId } from './inbox';

function reviewPanel(ref: ChangeRequestRef, title?: string): PanelSpec {
  return { id: itemId(ref), component: 'review', title: title ?? `${ref.project.split('/').pop() ?? ref.project} ${ref.number}`, renderer: 'always', params: { ...ref } };
}

export function openReview(ref: ChangeRequestRef, title?: string, split = false): void {
  useDock().openPanel(reviewPanel(ref, title), split);
}

export function reviveReview(id: string): PanelSpec | undefined {
  const match = /^mr:([^/]+)\/(.+)\/(\d+)$/.exec(id);
  return match?.[1] && match[2] && match[3] ? reviewPanel({ connector: match[1], project: match[2], number: Number(match[3]) }) : undefined;
}
