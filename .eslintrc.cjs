const path = require('path');

const uiNodeModules = path.join(__dirname, 'ui', 'node_modules');

function resolveFromUi(packageName) {
  return require.resolve(packageName, { paths: [uiNodeModules] });
}

module.exports = {
  root: true,
  env: {
    browser: true,
    es2022: true,
    node: true,
  },
  // Parsers resolve relative to this root config file; point them at ui/node_modules
  // so CI can keep a single `npm ci` under ui/.
  parser: resolveFromUi('@typescript-eslint/parser'),
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:react-hooks/recommended',
  ],
  parserOptions: {
    ecmaVersion: 'latest',
    sourceType: 'module',
    ecmaFeatures: {
      jsx: true,
    },
  },
  plugins: ['@typescript-eslint', 'react-hooks', 'react-refresh'],
  settings: {
    react: {
      version: 'detect',
    },
  },
  ignorePatterns: ['dist', 'node_modules', 'coverage'],
  rules: {
    'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
  },
};
