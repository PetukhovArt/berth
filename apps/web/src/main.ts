import './app/legacyStorage';
import '@fontsource-variable/wix-madefor-text';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { installUiKit } from './shared/ui/kit';
import './app/styles/app.css';
import './shared/ui/motion/statusAnimation.css';
import './app/actions';
import { initApp } from './app/init';
import WorkspaceView from './app/ui/WorkspaceView.vue';

initApp();
const app = createApp(WorkspaceView);
installUiKit(app);
app.mount('#app');
