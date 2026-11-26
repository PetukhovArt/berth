import { ref } from 'vue';
import type { ChangeRequestRef } from '../api/connectors/api';
import { withViewTransition } from '../ui/motion/motion';

export type View =
  | { kind: 'agents' }
  | { kind: 'reviews'; open?: ChangeRequestRef }
  | { kind: 'jira'; open?: { connector: string; key: string } }
  | { kind: 'confluence'; open?: { connector: string; id: string } }
  | { kind: 'settings' };

export const view = ref<View>({ kind: 'agents' });
export const requestedSettingsCategory = ref<string | null>(null);

export const viewBeforeSettings = ref<View | null>(null);

export function navigate(next: View): Promise<void> {
  return withViewTransition(() => {
    if (next.kind === 'settings' && view.value.kind !== 'settings') viewBeforeSettings.value = view.value;
    view.value = next;
  });
}

export function leaveSettings(): Promise<void> {
  const back = viewBeforeSettings.value;
  viewBeforeSettings.value = null;
  return navigate(back && back.kind !== 'settings' ? back : { kind: 'agents' });
}

export function openSettings(category?: string): void {
  requestedSettingsCategory.value = category ?? null;
  if (view.value.kind !== 'settings') void navigate({ kind: 'settings' });
}
