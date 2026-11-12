import js from '@eslint/js'
import ts from 'typescript-eslint'
import vue from 'eslint-plugin-vue'
import globals from 'globals'
export default ts.config(
    { ignores: ['node_modules/**', 'dist/**', 'playwright-report/**', 'test-results/**'] },
    js.configs.recommended,
    ...ts.configs.recommended,
    ...vue.configs['flat/essential'],
    {
        files: ['**/*.{ts,vue}'],
        languageOptions: {
            parserOptions: { parser: ts.parser },
            globals: { ...globals.browser, ...globals.node },
        },
        rules: {
            'vue/multi-word-component-names': [
                'error',
                {
                    ignores: [
                        'index',
                        'button',
                        'checkbox',
                        'dialog',
                        'drawer',
                        'input',
                        'pagination',
                        'select',
                        'tooltip',
                        'register',
                        '403',
                        '404',
                    ],
                },
            ],
            '@typescript-eslint/no-explicit-any': 'warn',
            '@typescript-eslint/no-unused-vars': [
                'error',
                { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
            ],
        },
    },
    { files: ['**/*.js'], languageOptions: { globals: globals.node } },
)
