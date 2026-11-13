import { shallowRef, watch } from 'vue';
import { useBerth, useSessionDock } from '../../../entities/session';
import { hostWindow, isDesktop } from '../../../bridge';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';

export type AttentionKind = 'waiting' | 'finished' | 'crashed';
type Kind = AttentionKind;
export type AttentionToast = { id: number; key: string; kind: Kind; text: string };

const TOAST_MS = 8000;
const TOAST_LIMIT = 4;

export const attentionToasts = shallowRef<readonly AttentionToast[]>([]);
let counter = 0;

export function dismissAttention(id: number): void {
  attentionToasts.value = attentionToasts.value.filter((toast) => toast.id !== id);
}

export function openAttention(toast: Pick<AttentionToast, 'id' | 'key'>): void {
  dismissAttention(toast.id);
  const node = useBerth()?.sessions.value.get(toast.key);
  if (node) useSessionDock().openSession(node);
  void hostWindow.raise().catch(() => undefined);
}

export function attentionKindOf(status: string, unseen: boolean): Kind | null {
  if (status === 'needs_feedback') return 'waiting';
  if (status === 'terminated') return 'crashed';
  if (status === 'finished' && unseen) return 'finished';
  return null;
}

function kindOf(previous: string | undefined, status: string, unseen: boolean): Kind | null {
  if (previous === status) return null;
  if (status === 'finished' && previous !== 'running') return null;
  return attentionKindOf(status, unseen);
}

function notifyOs(toast: AttentionToast): void {
  if (typeof Notification === 'undefined') return;
  const show = () => {
    const note = new Notification('Berth', { body: toast.text, tag: `berth-${toast.key}-${toast.kind}` });
    note.onclick = () => {
      note.close();
      openAttention(toast);
    };
  };
  if (Notification.permission === 'granted') show();
  else if (Notification.permission === 'default') void Notification.requestPermission().then((result) => result === 'granted' && show());
}

export function initAttention(): () => void {
  const berth = useBerth();
  if (!berth || !isDesktop) return () => undefined;
  const dock = useDock();
  const seen = new Map<string, string>();
  let ready = false;
  const stop = watch(
    () => berth.sessions.value,
    (sessions) => {
      for (const node of sessions.values()) {
        if (!('Agent' in node.ref)) continue;
        const previous = seen.get(node.key);
        seen.set(node.key, node.status);
        if (!ready) continue;
        const kind = kindOf(previous, node.status, node.unseen);
        if (!kind) continue;
        const focused = document.hasFocus();
        if (focused && dock.activeKey.value === node.key) continue;
        const toast: AttentionToast = { id: ++counter, key: node.key, kind, text: t(`attention.${kind}`, { name: node.name }) };
        if (!focused) {
          notifyOs(toast);
          continue;
        }
        attentionToasts.value = [...attentionToasts.value.filter((entry) => entry.key !== node.key), toast].slice(-TOAST_LIMIT);
        setTimeout(() => dismissAttention(toast.id), TOAST_MS);
      }
      ready = true;
    },
    { immediate: true, flush: 'sync' },
  );
  return stop;
}
