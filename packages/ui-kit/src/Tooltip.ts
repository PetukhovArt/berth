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

const forward = Object.fromEntries(
  HOOKS.map((hook) => [
    hook,
    (el: HTMLElement, binding: DirectiveBinding<Value>, vnode: VNode, prev: VNode | null) => {
      (Tooltip[hook] as ((...args: unknown[]) => void) | undefined)?.(el, normalize(binding), vnode, prev);
      if (hook === 'mounted' || hook === 'updated') name(el, binding.value);
    },
  ]),
);

export const vTooltip = forward as ObjectDirective<HTMLElement, Value>;
