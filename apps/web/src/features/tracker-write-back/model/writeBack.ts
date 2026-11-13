import { watch } from 'vue';
import { useBerth } from '../../../entities/session';
import { isDesktop } from '../../../bridge';
import { trackerComment } from '../../../shared/api/connectors/api';
import { useSettings } from '../../../shared/config/settings/store';
import { t } from '../../../shared/lib/i18n/i18n';

export function initTrackerWriteBack(): () => void {
  const berth = useBerth();
  if (!berth || !isDesktop) return () => undefined;
  const { values } = useSettings();
  const seen = new Map<string, string>();
  let ready = false;
  return watch(
    () => berth.sessions.value,
    (sessions) => {
      const worktrees = new Map<string, { path: string; branch: string }>();
      for (const workspace of berth.tree.value) for (const project of workspace.projects) for (const worktree of project.worktrees) for (const node of worktree.sessions) worktrees.set(node.key, worktree);
      for (const node of sessions.values()) {
        if (!('Agent' in node.ref)) continue;
        const previous = seen.get(node.key);
        seen.set(node.key, node.status);
        if (!ready || previous !== 'running' || node.status !== 'finished') continue;
        if (!values.value['connectors.trackerWriteBack']) continue;
        const worktree = worktrees.get(node.key);
        if (!worktree?.branch) continue;
        void trackerComment(worktree.path, worktree.branch, t('connectors.writeBack', { name: node.name, branch: worktree.branch })).catch(() => undefined);
      }
      ready = true;
    },
    { immediate: true, flush: 'sync' },
  );
}
