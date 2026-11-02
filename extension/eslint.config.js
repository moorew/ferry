// SPDX-License-Identifier: GPL-2.0-or-later

import {defineConfig} from '@eslint/config-helpers';
import gnome from 'eslint-config-gnome';

export default defineConfig([
    gnome.configs.recommended,
    gnome.configs.jsdoc,
    {
        ignores: ['node_modules'],
    }, {
        rules: {
            'jsdoc/require-jsdoc': ['error', {
                exemptEmptyFunctions: true,
                publicOnly: {esm: true},
            }],
            'jsdoc/require-param-description': 'off',
        },
        languageOptions: {
            globals: {global: 'readonly'},
        },
    },
]);
