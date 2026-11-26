import type { DirectiveBinding, ObjectDirective, VNode } from 'vue';
import Tooltip, { type TooltipOptions } from 'primevue/tooltip';

const SHOW_DELAY = 400;
const PLACED = ['top', 'bottom', 'left', 'right', 'focus'];
const NAMED = 'vrTooltipLabel';

type Value = string | false | null | undefined | TooltipOptions;
type Hook = 'created' | 'beforeMount' | 'mounted' | 'beforeUpdate' | 'updated' | 'beforeUnmount' | 'unmounted';

const HOOKS: Hook[] = ['created', 'beforeMount', 'mounted', 'beforeUpdate', 'updated', 'beforeUnmount', 'unmounted'];

const textOf = (value: Value) => (typeof value === 'string' ? value : value && typeof value === 'object' ? value.value : undefined);

const touch = () => typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches;

function normalize(binding: DirectiveBinding<Value>): DirectiveBinding<Value> {
  const { value, modifiers } = binding;
  const placed = PLACED.some((name) => modifiers[name]);
  return {
    ...binding,
    modifiers: placed ? modifiers : { ...modifiers, bottom: true },
    value: typeof value === 'string' ? { value, showDelay: SHOW_DELAY, disabled: touch() } : value && typeof value === 'object' ? { showDelay: SHOW_DELAY, disabled: touch(), ...value } : value,
  };
}

function name(el: HTMLElement, value: Value) {
  const text = textOf(value);
  const own = el.getAttribute('aria-label');
  if (!text) {
    if (own !== null && own === el.dataset[NAMED]) el.removeAttribute('aria-label');
    delete el.dataset[NAMED];
    return;
  }
  if ((own !== null && own !== el.dataset[NAMED]) || el.hasAttribute('aria-labelledby') || (el.textContent ?? '').trim()) return;
  el.setAttribute('aria-label', text);
  el.dataset[NAMED] = text;
}

const dismissers = new WeakMap<HTMLElement, () => void>();

const hide = (el: HTMLElement) => el.dispatchEvent(new MouseEvent('mouseleave'));

const expanded = (el: HTMLElement) => el.getAttribute('aria-expanded') === 'true';

function dismissOnPress(el: HTMLElement) {
  if (dismissers.has(el)) return;
  const dismiss = () => hide(el);
  const refuse = (event: Event) => {
    if (expanded(el)) event.stopImmediatePropagation();
  };
  el.addEventListener('pointerdown', dismiss);
  el.addEventListener('click', dismiss);
  el.addEventListener('mouseenter', refuse, true);
  el.addEventListener('focus', refuse, true);
  dismissers.set(el, () => {
    el.removeEventListener('pointerdown', dismiss);
    el.removeEventListener('click', dismiss);
    el.removeEventListener('mouseenter', refuse, true);
    el.removeEventListener('focus', refuse, true);
  });
}

const forward = Object.fromEntries(
  HOOKS.map((hook) => [
    hook,
    (el: HTMLElement, binding: DirectiveBinding<Value>, vnode: VNode, prev: VNode | null) => {
      (Tooltip[hook] as ((...args: unknown[]) => void) | undefined)?.(el, normalize(binding), vnode, prev);
      if (hook === 'mounted') dismissOnPress(el);
      if (hook === 'mounted' || hook === 'updated') name(el, binding.value);
      if (hook === 'updated' && expanded(el)) hide(el);
      if (hook === 'unmounted') {
        dismissers.get(el)?.();
        dismissers.delete(el);
      }
    },
  ]),
);

export const vTooltip = forward as ObjectDirective<HTMLElement, Value>;
