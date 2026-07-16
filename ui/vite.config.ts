import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: false,
    // Server-mode dev: run `cargo run -p finsight-server` (listens on :8674)
    // and plain `npm run dev` — the HTTP/SSE shim's /api calls proxy through.
    // Harmless when the server isn't running (only /api/* paths are proxied).
    proxy: {
      "/api": { target: "http://localhost:8674", changeOrigin: false },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    globals: true,
  },
});
