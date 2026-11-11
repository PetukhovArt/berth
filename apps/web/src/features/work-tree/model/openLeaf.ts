import { useSessionDock } from '../../../entities/session/model/sessionDock';
import { openReview } from '../../../entities/merge-request/model/reviewPanel';
import { openIssue } from '../../../entities/jira-issue/model/issuePanel';
import { numberLabel } from '../../../entities/merge-request/lib/format';
import type { WorkNode } from './tree';

export function openLeaf(node: WorkNode, split = false): void {
  const data = node.data;
  const sessions = useSessionDock();
  if (node.kind === 'session' && data?.session) sessions.openSession(data.session, undefined, false, split);
  else if (data?.external) sessions.openExternal(data.external, split);
  else if (data?.item) openReview(data.item, `${numberLabel(data.item)} ${data.item.title}`, split);
  else if (data?.issue) openIssue(data.issue.connector, data.issue.key, split);
}
