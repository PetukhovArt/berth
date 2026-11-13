import { watchEffect } from 'vue';
import { initKeymap } from '../shared/lib/commands/dispatcher';
import { t } from '../shared/lib/i18n/i18n';
import { setTranslator } from '../shared/lib/i18n/translate';
import { loadHostPlatform } from '../shared/lib/platform';
import { initAppearance } from '../shared/ui/appearance';
import { setMotionEnabled } from '../shared/ui/motion/motion';
import { initStatusAnimation } from '../shared/ui/motion/statusAnimation';
import { useSettings } from '../shared/config/settings/store';
import { initAttention } from '../features/attention';
import { initTrackerWriteBack } from '../features/tracker-write-back';
import { initArchiveOnMerge } from '../features/archive-on-merge';
import { initCleanup } from '../features/cleanup';
import { initAutoUpdate, initUpdate } from '../features/update';
import { initWorkTree } from '../features/work-tree';
import { initBerthIdleSync } from './berthIdle';
import { initLocale } from './locale';
import { initTray } from './tray';

export function initApp(): () => void {
  const disposers: (() => void)[] = [];
  disposers.push(initLocale());
  setTranslator(t);
  void loadHostPlatform();
  disposers.push(initAppearance());
  disposers.push(initBerthIdleSync());
  const { values } = useSettings();
  disposers.push(watchEffect(() => setMotionEnabled(values.value['ui.animations'])));
  disposers.push(initStatusAnimation(() => (values.value['ui.animations'] ? values.value['ui.statusAnimation'] : 'off')));
  disposers.push(initKeymap());
  disposers.push(initTray());
  disposers.push(initAttention());
  disposers.push(initTrackerWriteBack());
  disposers.push(initArchiveOnMerge());
  disposers.push(initCleanup());
  disposers.push(initUpdate());
  disposers.push(initAutoUpdate());
  disposers.push(initWorkTree());
  return () => {
    for (const dispose of disposers.splice(0).reverse()) dispose();
  };
}
