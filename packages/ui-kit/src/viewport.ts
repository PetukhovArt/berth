import { readonly, shallowRef, type Ref } from 'vue';
import { PHONE_QUERY } from '@ui/tokens';

const LONG_PRESS_MS = 650;
const LONG_PRESS_SLOP = 10;

const phone = shallowRef(false);

export function useIsPhone(): Readonly<Ref<boolean>> {
  return readonly(phone);
}

function trackViewport(root: HTMLElement) {
  const media = window.matchMedia(PHONE_QUERY);
  const apply = (matches: boolean) => {
    phone.value = matches;
    root.dataset.vrViewport = matches ? 'phone' : 'wide';
  };
  apply(media.matches);
  media.addEventListener('change', (event) => apply(event.matches));
}

function trackVisualViewport(root: HTMLElement) {
  const viewport = window.visualViewport;
  if (!viewport) return;
  const apply = () => {
    if (Math.abs(viewport.scale - 1) > 0.01) return;
    root.style.setProperty('--vr-app-height', `${Math.round(viewport.height)}px`);
    root.style.setProperty('--vr-app-top', `${Math.round(viewport.offsetTop)}px`);
  };
  apply();
  viewport.addEventListener('resize', apply);
  viewport.addEventListener('scroll', apply);
}

function installLongPress() {
  let timer = 0;
  let origin: { x: number; y: number } | null = null;
  let fired = false;
  const cancel = () => {
    window.clearTimeout(timer);
    origin = null;
  };
  document.addEventListener(
    'touchstart',
    (event) => {
      cancel();
      fired = false;
      const touch = event.touches[0];
      if (event.touches.length !== 1 || !touch || !(event.target instanceof Element)) return;
      const target = event.target;
      origin = { x: touch.clientX, y: touch.clientY };
      timer = window.setTimeout(() => {
        if (!origin) return;
        fired = true;
        target.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: origin.x, clientY: origin.y, button: 2 }));
        origin = null;
      }, LONG_PRESS_MS);
    },
    { passive: true },
  );
  document.addEventListener(
    'touchmove',
    (event) => {
      const touch = event.touches[0];
      if (origin && touch && Math.hypot(touch.clientX - origin.x, touch.clientY - origin.y) > LONG_PRESS_SLOP) cancel();
    },
    { passive: true },
  );
  document.addEventListener('touchcancel', cancel, { passive: true });
  document.addEventListener('contextmenu', (event) => {
    if (event.isTrusted) cancel();
  });
  document.addEventListener('touchend', (event) => {
    cancel();
    if (fired && event.cancelable) event.preventDefault();
    fired = false;
  });
}

export function installViewport(): void {
  const root = document.documentElement;
  trackViewport(root);
  trackVisualViewport(root);
  if (window.matchMedia('(hover: none)').matches) installLongPress();
}
