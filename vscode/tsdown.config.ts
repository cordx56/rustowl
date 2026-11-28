const { defineConfig } = require("tsdown");

/** @type {import('tsdown').UserConfig} */
module.exports = defineConfig({
  entry: ["src/extension.ts"],
  format: "cjs",
  platform: "node",
  deps: {
    neverBundle: ["vscode"],
  },
  sourcemap: true,
  outputOptions: {
    entryFileNames: "[name].js",
  },
});
