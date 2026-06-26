import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import boundaries from 'eslint-plugin-boundaries';

// Architecture tiers (the web-map feature-slice plan): app → host → features/* →
// map-core → shared, with map-engine (substrate) implementing map-core. Features
// are leaves: they may use the kernel + shared + their own slice, but never each
// other, the host, or the engine implementation. Enforced by element-types below.
export default tseslint.config(
  { ignores: ['dist', 'node_modules'] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2023,
      globals: globals.browser,
    },
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    plugins: { boundaries },
    settings: {
      // Resolve TS/TSX relative + index imports so boundaries can map each import
      // to its element (without this, deps resolve to null and no rule applies).
      'import/resolver': { typescript: { project: './tsconfig.json' } },
      'boundaries/include': ['src/**/*'],
      'boundaries/elements': [
        { type: 'app', mode: 'full', pattern: ['src/App.tsx', 'src/main.tsx'] },
        { type: 'host', mode: 'file', pattern: ['src/map/**/*'] },
        { type: 'feature', mode: 'full', pattern: ['src/features/*/**'], capture: ['feature'] },
        { type: 'engine', mode: 'file', pattern: ['src/map-engine/**/*'] },
        { type: 'core', mode: 'file', pattern: ['src/map-core/**/*'] },
        // Shared leaf modules: ui kit, api/data layer, cross-cutting stores, theme,
        // activity metadata, and the top-level geo/format/config helpers.
        { type: 'shared', mode: 'file', pattern: ['src/ui/**/*', 'src/api/**/*', 'src/store/**/*', 'src/theme/**/*', 'src/activities/**/*', 'src/geo.ts', 'src/format.ts', 'src/config.ts', 'src/baseLayers.ts'] },
      ],
    },
    rules: {
      // The legacy `element-types` rule is used (not the v6 `dependencies` rule):
      // its whitelist semantics + `${from.feature}` capture template are the
      // documented, working form. v6's `dependencies` allow/disallow with captured
      // self-matching did not reliably reject cross-feature imports here. It emits
      // a harmless "deprecated rule" warning; enforcement (errors) works.
      'boundaries/element-types': [
        'error',
        {
          default: 'disallow',
          // Each tier may import within itself (a tier is one cohesive module) +
          // the tiers below it; a feature may also import ITS OWN slice only.
          rules: [
            { from: ['app'], allow: ['app', 'host', 'feature', 'engine', 'core', 'shared'] },
            { from: ['host'], allow: ['host', 'feature', 'engine', 'core', 'shared'] },
            { from: ['feature'], allow: ['core', 'shared', ['feature', { feature: '${from.feature}' }]] },
            { from: ['engine'], allow: ['engine', 'core', 'shared'] },
            { from: ['core'], allow: ['core', 'shared'] },
            { from: ['shared'], allow: ['shared'] },
          ],
        },
      ],
    },
  },
);
