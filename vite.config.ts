import { defineConfig } from "vite";
import { resolve } from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "safari16",
    outDir: "dist",
    emptyOutDir: true,
    chunkSizeWarningLimit: 2000,
    rollupOptions: {
      input: {
        index: resolve(process.cwd(), "index.html"),
        notch: resolve(process.cwd(), "notch.html"),
        rapida: resolve(process.cwd(), "rapida.html"),
      },
    },
  },
});
