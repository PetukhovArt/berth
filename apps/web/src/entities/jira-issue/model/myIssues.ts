import { computed, shallowRef } from 'vue';
import { describeError, jiraSearch, onConnectorRecovered, useConnectors, type JiraIssueSummary } from '../../../shared/api/connectors/api';
import { visibleInterval, type Scope } from '../../../shared/lib/scope';

const JIRA_JQL = 'assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC';
const JIRA_LIMIT = 100;
const JIRA_FRESH_MS = 60_000;
const JIRA_POLL_MS = 2 * 60_000;

type JiraResult = { issues: JiraIssueSummary[]; error: string | null; loading: boolean; loadedAt: number };

const jiraResults = shallowRef<ReadonlyMap<string, JiraResult>>(new Map());

function patchJira(connector: string, patch: Partial<JiraResult>) {
  const previous = jiraResults.value.get(connector) ?? { issues: [], error: null, loading: false, loadedAt: 0 };
  jiraResults.value = new Map([...jiraResults.value, [connector, { ...previous, ...patch }]]);
}

async function loadJira(connector: string, force: boolean) {
  const current = jiraResults.value.get(connector);
  if (current?.loading || (!force && current && Date.now() - current.loadedAt < JIRA_FRESH_MS)) return;
  patchJira(connector, { loading: true });
  try {
    const result = await jiraSearch(connector, JIRA_JQL, 0, JIRA_LIMIT);
    patchJira(connector, { issues: result.issues, error: null, loading: false, loadedAt: Date.now() });
  } catch (e) {
    patchJira(connector, { error: describeError(e), loading: false, loadedAt: Date.now() });
  }
}

export function createMyIssues(scope: Scope) {
  const { connectors } = useConnectors();
  const jiraConnectors = computed(() => connectors.value.filter((c) => c.kind === 'jira'));
  scope.add(
    onConnectorRecovered((id) => {
      if (jiraResults.value.get(id)?.error) void loadJira(id, true);
    }),
  );
  visibleInterval(
    scope,
    JIRA_POLL_MS,
    () => {
      for (const connector of connectors.value) if (connector.kind === 'jira') void loadJira(connector.id, false);
    },
    false,
  );
  const issues = computed(() => jiraConnectors.value.flatMap((c) => jiraResults.value.get(c.id)?.issues ?? []));
  const errors = computed(() =>
    jiraConnectors.value.flatMap((c) => {
      const error = jiraResults.value.get(c.id)?.error;
      return error ? [{ connector: c.label, message: error }] : [];
    }),
  );
  function refresh() {
    for (const connector of jiraConnectors.value) void loadJira(connector.id, false);
  }

  return { jiraConnectors, issues, errors, refresh };
}
