import './app/legacyStorage';
import '@fontsource-variable/wix-madefor-text';
import '@fontsource-variable/jetbrains-mono';
import { createApp, type Component } from 'vue';
import { installUiKit } from './shared/ui/kit';
import './app/styles/app.css';
import './shared/ui/motion/statusAnimation.css';
import './app/actions';
import { initApp } from './app/init';
import { initBrowserLocale } from './app/locale';
import WorkspaceView from './app/ui/WorkspaceView.vue';

function mount(root: Component) {
  const app = createApp(root);
  installUiKit(app);
  app.mount('#app');
}

if (document.querySelector('meta[name="berth-share"]')) {
  initBrowserLocale();
  void import('./app/ui/SharedSessionView.vue').then((view) => mount(view.default));
} else {
  initApp();
  mount(WorkspaceView);
}
