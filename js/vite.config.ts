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
  build: {
    outDir: "../public/",
    emptyOutDir: true,
    rolldownOptions: {
      input: {
        admin: "src/admin.ts",
        "admin-shell": "src/admin-shell.ts",
        editor: "src/editor.ts",
        graph: "src/graph.ts",
        site: "src/site.ts",
      },
      output: {
        entryFileNames: "assets/[name].js",
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: "assets/[name][extname]",
        codeSplitting: {
          groups: [
            {
              name: "codemirror-view",
              test: /node_modules[\\/](@codemirror[\\/]view|style-mod|w3c-keyname|crelt)[\\/]/,
              priority: 5,
            },
            {
              name: "codemirror-state",
              test: /node_modules[\\/]@codemirror[\\/]state[\\/]/,
              priority: 5,
            },
            {
              name: "codemirror-language",
              test: /node_modules[\\/](@codemirror[\\/]language|@lezer[\\/](common|highlight|lr))[\\/]/,
              priority: 5,
            },
            {
              name: "codemirror-markdown",
              test: /node_modules[\\/](@codemirror[/]lang-markdown|@lezer[\\/]markdown)[\\/]/,
              priority: 5,
            },
            {
              name: "codemirror-editor",
              test: /node_modules[\\/](@codemirror[\\/](autocomplete|commands|lint|search)|codemirror)[\\/]/,
              priority: 5,
            },
            {
              name: "shared-vendor",
              test: /node_modules/,
              entriesAware: true,
              priority: 1,
            },
          ],
        },
      },
    },
  },
});
