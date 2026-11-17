import { shallowRef, watch } from 'vue';
import { attentionEdge, useBerth, useSessionDock, useSessionStates, type AttentionKind, type SessionStatus } from '../../../entities/session';
import { hostWindow, isDesktop } from '../../../bridge';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';

export type AttentionToast = { id: number; key: string; kind: AttentionKind; text: string };

const TOAST_MS = 8000;
const TOAST_LIMIT = 4;

export const attentionToasts = shallowRef<readonly AttentionToast[]>([]);
const listeners = new Set<(event: { key: string; agent: string | null; kind: AttentionKind; text: string }) => void>();

export function onAttention(listen: (event: { key: string; agent: string | null; kind: AttentionKind; text: string }) => void): () => void {
  listeners.add(listen);
  return () => listeners.delete(listen);
}
let counter = 0;

export function dismissAttention(id: number): void {
  attentionToasts.value = attentionToasts.value.filter((toast) => toast.id !== id);
}

export function openAttention(toast: Pick<AttentionToast, 'id' | 'key'>): void {
  dismissAttention(toast.id);
  useSessionDock().openKey(toast.key);
  void hostWindow.raise().catch(() => undefined);
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
  const states = useSessionStates();
  const seen = new Map<string, SessionStatus>();
  const stop = watch(
    () => states.value,
    (current) => {
      for (const state of current.values()) {
        const previous = seen.get(state.key);
        seen.set(state.key, state.status);
        if (previous === undefined) continue;
        const kind = attentionEdge(previous, state);
        if (!kind) continue;
        const focused = document.hasFocus();
        if (focused && dock.activeKey.value === state.key) continue;
        const toast: AttentionToast = { id: ++counter, key: state.key, kind, text: t(`attention.${kind}`, { name: state.name }) };
        if (!focused) {
          const node = berth.sessions.value.get(state.key);
          const agent = node && 'Agent' in node.ref ? node.ref.Agent : null;
          for (const listen of listeners) listen({ key: state.key, agent, kind, text: toast.text });
          notifyOs(toast);
          continue;
        }
        attentionToasts.value = [...attentionToasts.value.filter((entry) => entry.key !== state.key), toast].slice(-TOAST_LIMIT);
        setTimeout(() => dismissAttention(toast.id), TOAST_MS);
      }
    },
    { immediate: true, flush: 'sync' },
  );
  return stop;
}
