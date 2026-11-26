import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  root: here('.'),
  base: './',
  plugins: [vue()],
  resolve: { dedupe: ['vue', 'primevue', '@primeuix/themes'] },
  server: { port: Number(process.env.PORT ?? 5180), fs: { allow: [here('../../..')] } },
  build: {
    outDir: here('./dist'),
    emptyOutDir: true,
    cssCodeSplit: false,
    modulePreload: false,
    assetsInlineLimit: 0,
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
        entryFileNames: 'kit.js',
        chunkFileNames: 'kit.js',
        assetFileNames: (asset) => (asset.names.some((name) => name.endsWith('.css')) ? 'kit.css' : 'fonts/[name][extname]'),
      },
    },
  },
});
