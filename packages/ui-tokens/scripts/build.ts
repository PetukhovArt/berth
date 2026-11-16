import { mkdirSync, writeFileSync } from 'node:fs';
import StyleDictionary from 'style-dictionary';

const themes = [
  { name: 'light', selector: ':root' },
  { name: 'dark', selector: '.vr-dark' },
] as const;

const FONT_SCALED = new Set(['size', 'leading']);

StyleDictionary.registerTransform({
  name: 'vr/font-scale',
  type: 'value',
  filter: (token) => token.path[0] === 'font' && FONT_SCALED.has(token.path[1] ?? ''),
  transform: (token) => `calc(${String(token.$value ?? token.value)} * var(--vr-font-scale))`,
});

const config = (source: string[], destination: string, selector: string) => ({
  source,
  log: { verbosity: 'silent' as const },
  platforms: {
    css: {
      transforms: [...StyleDictionary.hooks.transformGroups.css, 'vr/font-scale'],
      prefix: 'vr',
      buildPath: 'generated/',
      files: [{ destination, format: 'css/variables', options: { selector, outputReferences: false } }],
    },
  },
});

for (const theme of themes) {
  const sd = new StyleDictionary(config(['tokens/primitive.json', `tokens/semantic.${theme.name}.json`], `${theme.name}.css`, theme.selector));
  await sd.buildAllPlatforms();
}

const render = async (source: string[], selector: string) => {
  const sd = new StyleDictionary(config(source, 'motion.css', selector));
  const [file] = await sd.formatPlatform('css');
  return String(file?.output ?? '').replace(/^\/\*[\s\S]*?\*\/\s*/, '').trim();
};

const motion = await render(['tokens/motion.json'], ':root');
const reduced = await render(['tokens/motion.reduced.json'], ':root.vr-reduce-motion');
const media = (await render(['tokens/motion.reduced.json'], ':root')).replace(/^/gm, '  ');
mkdirSync('generated', { recursive: true });
writeFileSync(
  'generated/motion.css',
  `/**\n * Do not edit directly, this file was auto-generated.\n */\n\n${motion}\n\n@media (prefers-reduced-motion: reduce) {\n${media}\n}\n\n${reduced}\n`,
);
