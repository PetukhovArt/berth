<script setup lang="ts">
import '@xterm/xterm/css/xterm.css';
import { onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { Terminal, type ITheme } from '@xterm/xterm';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebglAddon } from '@xterm/addon-webgl';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { attachSession, detachSession, resizeSession, sendInput, sessionId, type SessionEvent, type SessionRef } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { openUrl } from '../../../bridge';
import { hostPlatform } from '../../../shared/lib/platform';
import { applyCtrl } from '../lib/mobileKeys';
import { useSettings } from '../../../shared/config/settings/store';
import { terminalClipboard } from '../lib/terminalClipboard';
import { fitTerminal } from '../lib/terminalFit';
import { zoomTerminal } from '../lib/terminalFont';

type LiveTerminal = { show: () => void; hide: () => void; dispose: () => void };

const props = defineProps<{ session: SessionRef; panel?: DockviewPanelApi }>();
const host = useTemplateRef<HTMLDivElement>('host');
const { values } = useSettings();
const released = shallowRef(true);

const ESC = '\x1b';
const SHIFT_ENTER = `${ESC}\r`;
const CTRL_ENTER = `${ESC}[13;5u`;
const REPORT_BODY = /^(?:\??\d+;\d+R|[?>][\d;]*c|\??[\d;]*n)$/;
const RESIZE_DEBOUNCE_MS = 80;
const MINUTE_MS = 60_000;

const isTerminalReport = (data: string) => hostPlatform.value.capabilities.ptyAnswersTerminalQueries && data.startsWith(`${ESC}[`) && REPORT_BODY.test(data.slice(2));

function enterSequence(event: KeyboardEvent): string | null {
  if (event.key !== 'Enter' || event.altKey || event.metaKey || event.shiftKey === event.ctrlKey) return null;
  return event.shiftKey ? SHIFT_ENTER : CTRL_ENTER;
}

function decode(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function themeFromTokens(): ITheme {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string) => style.getPropertyValue(`--vr-color-${name}`).trim();
  const term = (name: string) => style.getPropertyValue(`--vr-term-${name}`).trim();
  return {
    background: token('surface-sunken'),
    foreground: term('fg'),
    cursor: term('fg'),
    cursorAccent: token('surface-sunken'),
    selectionBackground: token('nav-active'),
    brightBlack: term('dim'),
    red: term('red'),
    green: term('green'),
    yellow: term('accent'),
    blue: term('blue'),
  };
}

let detached: Promise<unknown> = Promise.resolve();

function build(element: HTMLElement, session: SessionRef): LiveTerminal {
  const term = new Terminal({
    allowProposedApi: true,
    fontFamily: values.value['terminal.font_family'],
    fontSize: values.value['terminal.font_size'],
    lineHeight: values.value['terminal.line_height'],
    cursorBlink: values.value['terminal.cursor_blink'],
    scrollback: values.value['terminal.scrollback'],
    macOptionIsMeta: values.value['terminal.option_as_meta'],
    theme: themeFromTokens(),
    linkHandler: {
      activate: (_event, uri) => {
        if (/^(?:https?|mailto):/i.test(uri)) void openUrl(uri);
      },
      allowNonHttpProtocols: true,
    },
  });
  term.loadAddon(new Unicode11Addon());
  term.unicode.activeVersion = '11';
  term.loadAddon(new WebLinksAddon((_event, uri) => void openUrl(uri)));
  term.open(element);
  const clipboard = terminalClipboard(term, element);

  let webgl: WebglAddon | null = null;
  const loadWebgl = () => {
    if (webgl) return;
    const addon = new WebglAddon();
    addon.onContextLoss(() => {
      if (webgl === addon) webgl = null;
      addon.dispose();
    });
    try {
      term.loadAddon(addon);
      webgl = addon;
    } catch {
      addon.dispose();
    }
  };
  const unloadWebgl = () => {
    webgl?.dispose();
    webgl = null;
  };
  loadWebgl();

  let disposed = false;
  let fromKeyboard = false;
  const input = (data: string) => void sendInput(session, applyCtrl(data));

  term.attachCustomKeyEventHandler((event) => {
    fromKeyboard = true;
    queueMicrotask(() => {
      fromKeyboard = false;
    });
    const sequence = enterSequence(event);
    if (sequence) {
      if (event.type === 'keydown') {
        event.preventDefault();
        input(sequence);
      }
      return false;
    }
    return clipboard.handleKey(event) ?? true;
  });

  term.onData((data) => {
    if (!fromKeyboard && isTerminalReport(data)) return;
    input(data);
  });
  term.onResize(({ cols, rows }) => void resizeSession(session, cols, rows));

  const refit = () => {
    fitTerminal(term, element);
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const resizes = new ResizeObserver(() => {
    clearTimeout(timer);
    timer = setTimeout(refit, RESIZE_DEBOUNCE_MS);
  });
  resizes.observe(element);
  const themes = new MutationObserver(() => {
    term.options.theme = themeFromTokens();
  });
  themes.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });

  const stopLayout = watch(
    () => [values.value['terminal.font_family'], values.value['terminal.font_size'], values.value['terminal.line_height']] as const,
    ([fontFamily, fontSize, lineHeight]) => {
      term.options.fontFamily = fontFamily;
      term.options.fontSize = fontSize;
      term.options.lineHeight = lineHeight;
      refit();
    },
  );
  const stopBehaviour = watch(
    () => [values.value['terminal.cursor_blink'], values.value['terminal.scrollback'], values.value['terminal.option_as_meta']] as const,
    ([cursorBlink, scrollback, optionAsMeta]) => {
      term.options.cursorBlink = cursorBlink;
      term.options.scrollback = scrollback;
      term.options.macOptionIsMeta = optionAsMeta;
    },
  );
  const onWheel = (event: WheelEvent) => {
    if (!event.ctrlKey || event.deltaY === 0) return;
    event.preventDefault();
    event.stopPropagation();
    zoomTerminal(event.deltaY < 0 ? 1 : -1);
  };
  element.addEventListener('wheel', onWheel, { passive: false, capture: true });

  refit();
  const onEvent = (event: SessionEvent) => {
    if (disposed) return;
    switch (event.type) {
      case 'reset':
        term.reset();
        term.write(decode(event.data));
        return;
      case 'output':
        term.write(decode(event.data));
        return;
      case 'exited':
        term.write(`\r\n${ESC}[2m[${event.exit_code === null ? t('agents.session.exited') : t('agents.session.exitedWithCode', { code: event.exit_code })}]${ESC}[0m\r\n`);
        return;
      default: {
        const unknown: never = event;
        return unknown;
      }
    }
  };
  const attached = detached.then(() => attachSession(session, term.cols, term.rows, onEvent));
  term.focus();

  return {
    show: () => {
      loadWebgl();
      refit();
      term.refresh(0, term.rows - 1);
    },
    hide: unloadWebgl,
    dispose: () => {
      disposed = true;
      clearTimeout(timer);
      resizes.disconnect();
      themes.disconnect();
      stopLayout();
      stopBehaviour();
      clipboard.dispose();
      element.removeEventListener('wheel', onWheel, { capture: true });
      detached = attached.then(() => detachSession(session)).catch(() => undefined);
      term.dispose();
    },
  };
}

let live: LiveTerminal | null = null;
let shown = false;
let evictTimer: ReturnType<typeof setTimeout> | undefined;
let observer: IntersectionObserver | undefined;
let inView = false;
let tabVisible = true;
let visibility: { dispose(): void } | undefined;

function armEviction() {
  clearTimeout(evictTimer);
  const minutes = values.value['terminal.evictHiddenAfter'];
  if (!live || shown || minutes <= 0) return;
  evictTimer = setTimeout(() => {
    if (shown || !live) return;
    live.dispose();
    live = null;
    released.value = true;
  }, minutes * MINUTE_MS);
}

function setShown(next: boolean) {
  if (next === shown) return;
  shown = next;
  const element = host.value;
  if (!element) return;
  if (next) {
    clearTimeout(evictTimer);
    if (live) live.show();
    else {
      live = build(element, props.session);
      released.value = false;
    }
    return;
  }
  live?.hide();
  armEviction();
}

onMounted(() => {
  const element = host.value;
  if (!element) return;
  const sync = () => setShown(inView && tabVisible);
  tabVisible = props.panel?.isVisible ?? true;
  visibility = props.panel?.onDidVisibilityChange((event) => {
    tabVisible = event.isVisible;
    sync();
  });
  observer = new IntersectionObserver((entries) => {
    const latest = entries.at(-1);
    if (!latest) return;
    inView = latest.isIntersecting;
    sync();
  });
  observer.observe(element);
});

watch(() => values.value['terminal.evictHiddenAfter'], armEviction);

onBeforeUnmount(() => {
  observer?.disconnect();
  visibility?.dispose();
  clearTimeout(evictTimer);
  live?.dispose();
  live = null;
});
</script>

<template>
  <div class="frame">
    <div ref="host" class="terminal" data-testid="session-terminal" :data-session-id="sessionId(session)" :data-released="released" />
    <p v-if="released" class="released" data-testid="session-terminal-released">{{ t('agents.session.released') }}</p>
  </div>
</template>

<style scoped>
.frame {
  position: relative;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  padding: var(--vr-space-1) 0 0 var(--vr-space-2);
  background: var(--vr-color-surface-sunken);
}

.terminal {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.released {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0;
  padding: var(--vr-space-4);
  color: var(--vr-color-text-muted);
  text-align: center;
  pointer-events: none;
}
</style>
