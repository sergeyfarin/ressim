const tsParser = require('@typescript-eslint/parser');
const svelte = require('eslint-plugin-svelte');

module.exports = [
  {
    ignores: ['dist/', 'node_modules/', 'coverage/', 'src/lib/ressim/pkg/'],
  },
  ...svelte.configs['flat/base'],
  {
    files: ['**/*.ts'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 2020,
        sourceType: 'module',
        project: './tsconfig.json',
      },
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: tsParser,
      },
    },
  },
  {
    files: ['src/**/*.{js,ts,svelte}'],
    rules: {
      'no-restricted-syntax': [
        'error',
        {
          selector: "MemberExpression[object.object.name='chart'][object.property.name='datasets'][computed=true]",
          message:
            "Direct index access to `chart.data.datasets[...]` is disallowed — use helpers from `src/lib/chart-helpers.ts`.",
        },
        {
          selector: "CallExpression[callee.property.name='forEach'][callee.object.object.object.name='chart'][callee.object.object.property.name='datasets']",
          message:
            "Direct iteration over `chart.data.datasets` is disallowed — use helpers from `src/lib/chart-helpers.ts`.",
        },
      ],
    },
  },
];
