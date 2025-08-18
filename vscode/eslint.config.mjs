// @ts-check

import { fileURLToPath } from "node:url";

import { includeIgnoreFile } from "@eslint/compat";
import eslint from "@eslint/js";
import typescriptEslint from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import { importX } from "eslint-plugin-import-x";
import jsdocPlugin from "eslint-plugin-jsdoc";
import nodePlugin from "eslint-plugin-n";
import oxlint from "eslint-plugin-oxlint";
import promisePlugin from "eslint-plugin-promise";
import securityPlugin from "eslint-plugin-security";
import globals from "globals";
import tseslint from "typescript-eslint";

const gitignorePath = fileURLToPath(new URL(".gitignore", import.meta.url));

// Shared rule sets
const sharedRules = {
  // Import rules
  "import-x/order": [
    "warn",
    {
      groups: ["builtin", "external", "internal", "parent", "sibling", "index"],
      "newlines-between": "always",
      alphabetize: { order: "asc", caseInsensitive: true },
    },
  ],
  "import-x/no-unresolved": "error",
  "import-x/no-cycle": "error",
  "import-x/no-duplicates": "warn",
  "import-x/first": "warn",
  "import-x/newline-after-import": "warn",
  "import-x/no-absolute-path": "error",
  "import-x/no-dynamic-require": "warn",
  "import-x/no-self-import": "error",
  "import-x/no-useless-path-segments": "warn",

  // Security rules
  "security/detect-buffer-noassert": "warn",
  "security/detect-child-process": "warn",
  "security/detect-eval-with-expression": "warn",
  "security/detect-non-literal-fs-filename": "warn",
  "security/detect-non-literal-regexp": "warn",
  "security/detect-non-literal-require": "warn",
  "security/detect-object-injection": "warn",
  "security/detect-unsafe-regex": "warn",

  // Promise rules
  "promise/always-return": "error",
  "promise/no-return-wrap": "error",
  "promise/param-names": "error",
  "promise/catch-or-return": "error",
  "promise/no-native": "off",
  "promise/no-nesting": "warn",
  "promise/no-promise-in-callback": "warn",
  "promise/no-callback-in-promise": "warn",
  "promise/avoid-new": "off",
  "promise/no-new-statics": "error",
  "promise/no-return-in-finally": "warn",
  "promise/valid-params": "warn",
  "promise/prefer-await-to-then": "warn",
  "promise/prefer-await-to-callbacks": "warn",

  // Node rules
  "n/no-deprecated-api": "warn",
  "n/no-extraneous-import": "error",
  "n/no-missing-import": "off",
  "n/no-unpublished-import": "off",
  "n/process-exit-as-throw": "error",
  "n/no-process-exit": "error",

  // General rules
  curly: "warn",
  eqeqeq: "warn",
  "no-throw-literal": "warn",
  semi: "warn",
  "no-console": "warn",
  "no-debugger": "warn",
  "no-duplicate-imports": "off",
  "no-unused-expressions": "warn",
  "prefer-const": "warn",
  "no-var": "error",
  "object-shorthand": "warn",
  "prefer-arrow-callback": "warn",
  "prefer-template": "warn",
  "quote-props": ["warn", "as-needed"],
  quotes: ["warn", "double", { avoidEscape: true }],
  "no-trailing-spaces": "warn",
  "eol-last": "warn",
  "comma-dangle": ["warn", "always-multiline"],
  "max-len": ["warn", { code: 120, ignoreUrls: true }],
  "brace-style": ["warn", "1tbs", { allowSingleLine: true }],
  "space-before-function-paren": [
    "warn",
    {
      anonymous: "always",
      named: "never",
      asyncArrow: "always",
    },
  ],
  "no-eval": "error",
  "no-implied-eval": "error",
  "no-new-func": "error",
  "no-script-url": "error",
  "no-caller": "error",
  "no-extend-native": "error",
  "no-extra-bind": "warn",
  "no-invalid-this": "error",
  "no-multi-spaces": "warn",
  "no-multi-str": "warn",
  "no-new-wrappers": "warn",
  "no-octal-escape": "error",
  "no-self-compare": "error",
  "no-sequences": "error",
  "no-unmodified-loop-condition": "warn",
  "no-unused-labels": "warn",
  "no-useless-call": "warn",
  "no-useless-concat": "warn",
  "no-useless-escape": "warn",
  "no-void": "warn",
  "no-with": "error",
  radix: "warn",
  "wrap-iife": ["warn", "any"],
  yoda: "warn",
};

export default tseslint.config(
  eslint.configs.recommended,
  includeIgnoreFile(gitignorePath, "Imported .gitignore patterns"),

  // TypeScript files
  {
    files: ["**/*.ts", "**/*.tsx"],
    plugins: {
      "@typescript-eslint": typescriptEslint,
      "import-x": importX,
      security: securityPlugin,
      promise: promisePlugin,
      n: nodePlugin,
      jsdoc: jsdocPlugin,
    },
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
      parser: tsParser,
      ecmaVersion: 2022,
      sourceType: "module",
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    settings: {
      "import-x/resolver": {
        typescript: { alwaysTryTypes: true, project: "./tsconfig.json" },
        node: true,
      },
    },
    rules: {
      ...tseslint.configs.recommendedTypeChecked.rules,
      ...sharedRules,

      // TypeScript-specific rules
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-inferrable-types": "warn",
      "@typescript-eslint/no-non-null-assertion": "warn",
      "@typescript-eslint/prefer-nullish-coalescing": "warn",
      "@typescript-eslint/prefer-optional-chain": "warn",
      "@typescript-eslint/no-unnecessary-type-assertion": "warn",
      "@typescript-eslint/no-floating-promises": "error",
      "@typescript-eslint/await-thenable": "error",
      "@typescript-eslint/no-misused-promises": "error",
      "@typescript-eslint/strict-boolean-expressions": "warn",
      "@typescript-eslint/prefer-readonly": "warn",

      // TypeScript-specific import rules
      "import-x/no-unused-modules": "warn",
      "import-x/no-deprecated": "warn",
      "import-x/consistent-type-specifier-style": ["warn", "prefer-top-level"],
      "import-x/no-empty-named-blocks": "warn",
      "import-x/no-extraneous-dependencies": [
        "error",
        {
          devDependencies: [
            "**/*.test.ts",
            "**/*.spec.ts",
            "**/test/**/*.ts",
            "**/tests/**/*.ts",
            "**/__tests__/**/*.ts",
            "esbuild.js",
            "eslint.config.mjs",
            "**/webpack.config.js",
            "**/rollup.config.js",
            "**/vite.config.ts",
          ],
        },
      ],

      // Additional security rules for TypeScript
      "security/detect-disable-mustache-escape": "warn",
      "security/detect-no-csrf-before-method-override": "warn",
      "security/detect-possible-timing-attacks": "warn",
      "security/detect-pseudoRandomBytes": "warn",

      // JSDoc rules
      "jsdoc/check-alignment": "warn",
      "jsdoc/check-indentation": "warn",
      "jsdoc/check-param-names": "warn",
      "jsdoc/check-syntax": "warn",
      "jsdoc/check-tag-names": "warn",
      "jsdoc/check-types": "warn",
      "jsdoc/no-undefined-types": "warn",
      "jsdoc/require-description": "warn",
      "jsdoc/require-description-complete-sentence": "warn",
      "jsdoc/require-hyphen-before-param-description": "warn",
      "jsdoc/require-param": "warn",
      "jsdoc/require-param-description": "warn",
      "jsdoc/require-param-name": "warn",
      "jsdoc/require-param-type": "off",
      "jsdoc/require-returns": "warn",
      "jsdoc/require-returns-description": "warn",
      "jsdoc/require-returns-type": "off",
    },
  },

  // JavaScript files
  {
    files: ["**/*.js", "**/*.mjs"],
    plugins: {
      "import-x": importX,
      security: securityPlugin,
      promise: promisePlugin,
      n: nodePlugin,
    },
    languageOptions: {
      globals: { ...globals.node },
      ecmaVersion: 2022,
      sourceType: "module",
    },
    settings: {
      "import-x/resolver": { node: true },
    },
    rules: sharedRules,
  },

  // Test files
  {
    files: ["**/*.test.ts", "**/*.spec.ts", "**/test/**/*.ts"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
      "no-console": "off",
      "@typescript-eslint/no-non-null-assertion": "off",
      "@typescript-eslint/no-unused-expressions": "off",
      "no-unused-expressions": "off",
      "jsdoc/require-description": "off",
      "jsdoc/require-returns": "off",
      "security/detect-non-literal-fs-filename": "off",
      "import-x/no-extraneous-dependencies": "off",
      "promise/always-return": "off",
    },
  },

  ...oxlint.buildFromOxlintConfigFile(".oxlintrc.json"),
);
