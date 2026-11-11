import { onScopeDispose } from 'vue';

export type Scope = {
  add(dispose: () => void): void;
  load<T>(promise: Promise<T>): Promise<T>;
  dispose(): void;
  readonly disposed: boolean;
};

export function createScope(): Scope {
  const disposers: (() => void)[] = [];
  let disposed = false;
  return {
    add(dispose) {
      if (disposed) dispose();
      else disposers.push(dispose);
    },
    load<T>(promise: Promise<T>): Promise<T> {
      return new Promise<T>((resolve, reject) => {
        promise.then(
          (value) => {
            if (!disposed) resolve(value);
          },
          (error: unknown) => {
            if (!disposed) reject(error);
          },
        );
      });
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const dispose of disposers.splice(0).reverse()) dispose();
    },
    get disposed() {
      return disposed;
    },
  };
}

export function useScope(): Scope {
  const scope = createScope();
  onScopeDispose(() => scope.dispose());
  return scope;
}

export function visibleInterval(scope: Scope, ms: number, run: () => void, catchUp = true): void {
  let missed = false;
  const timer = setInterval(() => {
    if (document.visibilityState === 'hidden') missed = catchUp;
    else run();
  }, ms);
  const onVisible = () => {
    if (document.visibilityState === 'hidden' || !missed) return;
    missed = false;
    run();
  };
  document.addEventListener('visibilitychange', onVisible);
  scope.add(() => {
    clearInterval(timer);
    document.removeEventListener('visibilitychange', onVisible);
  });
}
