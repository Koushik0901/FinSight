import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "coverage/**",
      "dist/**",
      "src/api/bindings.ts",
    ],
  },
  {
    languageOptions: {
      ecmaVersion: "latest",
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },
  js.configs.recommended,
  ...tseslint.configs.recommended.map((config) => ({
    ...config,
    files: ["**/*.{ts,tsx}"],
  })),
  {
    // ag-ui/Tauri copilot adapters are interim code written against a moving
    // @ag-ui surface; the @ts-nocheck at their top is deliberate and tracked as
    // typing debt (snake_case envelope adapters), not an accidental escape.
    files: [
      "src/components/copilot/TauriRuntime.ts",
      "src/components/copilot/streamFrame.ts",
      "src/components/copilot/agUi/**",
    ],
    rules: {
      "@typescript-eslint/ban-ts-comment": ["error", { "ts-nocheck": false }],
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      "no-undef": "off",
      "no-unused-vars": "off",
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": ["error", {
        argsIgnorePattern: "^_",
        caughtErrorsIgnorePattern: "^_",
        varsIgnorePattern: "^_",
      }],
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      // FinSight intentionally colocates small data helpers with several screens.
      // Hook correctness is enforced above; HMR module shape is not a source defect.
      "react-refresh/only-export-components": "off",
    },
  },
);
