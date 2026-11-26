import { createApp } from 'vue';
import '@fontsource-variable/jetbrains-mono';
import { installUiKit } from '../src';
import { kitLocale } from '../../../apps/web/src/app/kitLocale';
import '../../../apps/web/src/app/styles/app.css';
import './gallery.css';
import Gallery from './Gallery.vue';
import { installPseudoStates } from './pseudoStates';

const app = createApp(Gallery);
installUiKit(app, kitLocale);
app.mount('#app');
installPseudoStates();
