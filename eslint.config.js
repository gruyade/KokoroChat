import js from '@eslint/js';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import reactHooksPlugin from 'eslint-plugin-react-hooks';

export default [
  {
    ignores: ['dist/', 'src-tauri/', 'node_modules/', '*.config.ts', '*.config.js'],
  },
  js.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: {
          jsx: true,
        },
      },
      globals: {
        document: 'readonly',
        window: 'readonly',
        console: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        setInterval: 'readonly',
        clearInterval: 'readonly',
        fetch: 'readonly',
        URL: 'readonly',
        HTMLElement: 'readonly',
        HTMLDivElement: 'readonly',
        HTMLInputElement: 'readonly',
        HTMLTextAreaElement: 'readonly',
        Event: 'readonly',
        WheelEvent: 'readonly',
        TouchEvent: 'readonly',
        KeyboardEvent: 'readonly',
        MouseEvent: 'readonly',
        RequestInit: 'readonly',
        Response: 'readonly',
        File: 'readonly',
        FileReader: 'readonly',
        Image: 'readonly',
        localStorage: 'readonly',
        alert: 'readonly',
        confirm: 'readonly',
        requestAnimationFrame: 'readonly',
        cancelAnimationFrame: 'readonly',
        AudioContext: 'readonly',
        AudioBufferSourceNode: 'readonly',
        GainNode: 'readonly',
        Blob: 'readonly',
        FormData: 'readonly',
        AbortController: 'readonly',
        TextDecoder: 'readonly',
        React: 'readonly',
        navigator: 'readonly',
        Node: 'readonly',
        Element: 'readonly',
        structuredClone: 'readonly',
        atob: 'readonly',
        crypto: 'readonly',
      },
    },
    plugins: {
      '@typescript-eslint': tsPlugin,
      'react-hooks': reactHooksPlugin,
    },
    rules: {
      ...tsPlugin.configs.recommended.rules,
      ...reactHooksPlugin.configs.recommended.rules,
      '@typescript-eslint/no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/no-explicit-any': 'warn',
      'no-unused-vars': 'off',
    },
  },
];
