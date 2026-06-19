import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  // 忽略目录
  {
    ignores: ['dist', 'node_modules', 'eslint.config.js'],
  },

  // 基础推荐配置：检查未使用变量、未定义变量、空代码块等明显问题
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.browser,
        ...globals.es2022,
      },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      // React Hooks 规则
      ...reactHooks.configs.recommended.rules,

      // React Refresh：只允许导出组件（HMR 友好）
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],

      // 未使用变量：允许以 _ 开头的参数
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],

      // 允许使用 any（渐进式迁移）
      '@typescript-eslint/no-explicit-any': 'off',

      // 空函数体允许（回调占位）
      '@typescript-eslint/no-empty-function': 'off',
    },
  },
)
