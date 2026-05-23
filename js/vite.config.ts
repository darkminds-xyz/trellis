import { defineConfig } from "vite";
import { fileURLToPath, URL } from "node:url";

const modulePath = (name: string) =>
  fileURLToPath(new URL(`./node_modules/${name}`, import.meta.url));

const codeMirrorPackages = [
  "@codemirror/autocomplete",
  "@codemirror/commands",
  "@codemirror/lang-markdown",
  "@codemirror/language",
  "@codemirror/language-data",
  "@codemirror/lint",
  "@codemirror/search",
  "@codemirror/state",
  "@codemirror/view",
  "codemirror",
];

const lezerPackages = [
  "@lezer/common",
  "@lezer/highlight",
  "@lezer/lr",
  "@lezer/markdown",
  "@lezer/yaml",
];

export default defineConfig({
  resolve: {
    dedupe: [...codeMirrorPackages, ...lezerPackages],
    alias: Object.fromEntries([
      ...[...codeMirrorPackages, ...lezerPackages].map((name) => [
        name,
        modulePath(name),
      ]),
      ["@prosemark/core", modulePath("@tmark/core")],
    ]),
  },
  build: {
    outDir: "public/assets",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        admin: "src/admin.ts",
        editor: "src/editor.ts",
        graph: "src/graph.ts",
        styles: "src/styles.ts",
      },
      output: {
        entryFileNames: "[name].js",
        assetFileNames: "[name][extname]",
      },
    },
  },
});
