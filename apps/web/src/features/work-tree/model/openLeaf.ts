import { useSessionDock } from '../../../entities/session';
import { openReview, numberLabel } from '../../../entities/merge-request';
import { openIssue } from '../../../entities/jira-issue';
import type { WorkNode } from './tree';

export function openLeaf(node: WorkNode, split = false): void {
  const data = node.data;
  const sessions = useSessionDock();
  if (node.kind === 'session' && data?.session) sessions.openSession(data.session, undefined, false, split);
  else if (data?.external) sessions.openExternal(data.external, split);
  else if (data?.item) openReview(data.item, `${numberLabel(data.item)} ${data.item.title}`, split);
  else if (data?.issue) openIssue(data.issue.connector, data.issue.key, split);
}
