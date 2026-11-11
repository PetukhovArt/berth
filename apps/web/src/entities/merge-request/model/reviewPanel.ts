import { useDock } from '../../../shared/model/dock';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { itemId } from './inbox';

export function openReview(ref: ChangeRequestRef, title?: string, split = false): void {
  useDock().openPanel({ id: itemId(ref), component: 'review', title: title ?? `${ref.project.split('/').pop() ?? ref.project} ${ref.number}`, renderer: 'always', params: { ...ref } }, split);
}
