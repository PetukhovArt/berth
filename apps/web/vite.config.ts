import { readFileSync } from 'node:fs';
import { defineConfig, type Plugin } from 'vite';
import vue from '@vitejs/plugin-vue';

const tokenCss = (name: string) => readFileSync(new URL(`../../packages/ui-tokens/generated/${name}.css`, import.meta.url), 'utf8');

const splashTokens = (): Plugin => ({
  name: 'berth-splash-tokens',
  transformIndexHtml: () => [{ tag: 'style', children: ['light', 'dark'].map(tokenCss).join('\n'), injectTo: 'head-prepend' }],
});

export default defineConfig({
  plugins: [vue(), splashTokens()],
  server: {
    port: Number(process.env.PORT ?? 5173),
  },
});
