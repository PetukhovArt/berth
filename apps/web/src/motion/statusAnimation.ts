import { watchEffect } from 'vue';

const PREFIX = 'status-';

export function installStatusAnimation(variant: () => string): void {
  watchEffect(() => {
    document.documentElement.dataset.statusAnimation = variant();
  });
  document.addEventListener('animationstart', (event) => {
    if (!event.animationName.startsWith(PREFIX) || !(event.target instanceof Element)) return;
    for (const animation of event.target.getAnimations({ subtree: true }))
      if (animation instanceof CSSAnimation && animation.animationName.startsWith(PREFIX)) animation.startTime = 0;
  });
}
