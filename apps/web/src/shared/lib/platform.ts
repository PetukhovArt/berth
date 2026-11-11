import { shallowRef } from 'vue';
import { hasBackend, invoke } from '../../bridge';

type Os = 'windows' | 'macos' | 'linux';

type HostPlatform = {
  readonly os: Os;
  readonly capabilities: {
    readonly ptyAnswersTerminalQueries: boolean;
    readonly consoleDropsKittyInput: boolean;
    readonly pasteArrivesAsKeys: boolean;
    readonly win32ConsoleKeys: boolean;
    readonly pathsCaseInsensitive: boolean;
    readonly systemSounds: boolean;
  };
  readonly selfUpdate: boolean;
  readonly windowChrome: { readonly nativeDecorations: boolean; readonly nativeMenu: boolean };
};

function guessOs(): Os {
  const hints = navigator as Navigator & { userAgentData?: { platform?: string } };
  const source = `${hints.userAgentData?.platform ?? ''} ${navigator.userAgent}`;
  if (/windows/i.test(source)) return 'windows';
  if (/mac|iphone|ipad|ipod/i.test(source)) return 'macos';
  return 'linux';
}

const clientOs: Os = guessOs();

export const isMac = clientOs === 'macos';

export const modifierGlyphs = isMac
  ? ({ ctrl: '⌃', alt: '⌥', shift: '⇧', meta: '⌘' } as const)
  : ({ ctrl: 'Ctrl', alt: 'Alt', shift: 'Shift', meta: 'Meta' } as const);

export const chordJoiner = isMac ? '' : '+';

export const primaryModifier: 'ctrl' | 'meta' = isMac ? 'meta' : 'ctrl';

type ModifierState = Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>;

export function primaryPressed(event: ModifierState): boolean {
  return isMac ? event.metaKey : event.ctrlKey;
}

export function onlyPrimaryPressed(event: ModifierState, shift = false): boolean {
  return primaryPressed(event) && !event.altKey && event.shiftKey === shift && (isMac ? !event.ctrlKey : !event.metaKey);
}

const windowsHost = clientOs === 'windows';

export const hostPlatform = shallowRef<HostPlatform>({
  os: clientOs,
  capabilities: {
    ptyAnswersTerminalQueries: windowsHost,
    consoleDropsKittyInput: windowsHost,
    pasteArrivesAsKeys: windowsHost,
    win32ConsoleKeys: windowsHost,
    pathsCaseInsensitive: windowsHost,
    systemSounds: isMac,
  },
  selfUpdate: false,
  windowChrome: { nativeDecorations: false, nativeMenu: false },
});

let loading: Promise<HostPlatform> | null = null;

export function loadHostPlatform(): Promise<HostPlatform> {
  loading ??= hasBackend
    ? invoke<HostPlatform>('platform_info')
        .then((info) => (hostPlatform.value = info))
        .catch(() => hostPlatform.value)
    : Promise.resolve(hostPlatform.value);
  return loading;
}
