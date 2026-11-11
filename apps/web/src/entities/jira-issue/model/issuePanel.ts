import { useDock } from '../../../shared/model/dock';

export const issuePanelId = (connector: string, key: string) => `jira:${connector}/${key}`;

export function openIssue(connector: string, key: string, split = false): void {
  useDock().openPanel({ id: issuePanelId(connector, key), component: 'issue', title: key, params: { connector, key } }, split);
}
