import { watchEffect } from 'vue';

const PREFIX = 'status-';

function alignPhase(event: AnimationEvent): void {
  const target = event.target;
  if (!event.animationName.startsWith(PREFIX) || !(target instanceof Element)) return;
  for (const animation of target.getAnimations({ subtree: true })) {
    if (!(animation instanceof CSSAnimation) || !animation.animationName.startsWith(PREFIX) || animation.startTime === 0) continue;
    const effect = animation.effect;
    if (effect instanceof KeyframeEffect && effect.target === target) animation.startTime = 0;
  }
}

export function initStatusAnimation(variant: () => string): () => void {
  const stop = watchEffect(() => {
    document.documentElement.dataset.statusAnimation = variant();
  });
  const onStart = alignPhase;
  document.addEventListener('animationstart', onStart);
  return () => {
    stop();
    document.removeEventListener('animationstart', onStart);
  };
}
