import { defineConfig } from "vite";

export default defineConfig({
  build: {
    outDir: "public/assets",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        editor: "src/editor.js",
        styles: "src/styles.js",
      },
      output: {
        entryFileNames: "[name].js",
        assetFileNames: "[name][extname]",
      },
    },
  },
});
