import { computed, shallowRef, type ComputedRef } from 'vue';
import type { Harness } from './external';

export type MessageFocus = { harness: Harness | 'acp'; session: string; key: string; words: readonly string[] };

const pending = shallowRef<MessageFocus | null>(null);

export function focusMessage(focus: MessageFocus): void {
  pending.value = { ...focus };
}

export function messageFocusOf(harness: Harness | 'acp', session: string): ComputedRef<MessageFocus | null> {
  return computed(() => {
    const focus = pending.value;
    return focus && focus.harness === harness && focus.session === session ? focus : null;
  });
}

export function clearMessageFocus(focus: MessageFocus): void {
  if (pending.value === focus) pending.value = null;
}
