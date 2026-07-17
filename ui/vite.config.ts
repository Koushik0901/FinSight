import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: "autoUpdate",
      includeAssets: ["logo.svg"],
      pwaAssets: { image: "public/logo.svg" }, // generates 192/512/maskable/apple-touch icons
      manifest: {
        name: "FinSight",
        short_name: "FinSight",
        description: "Private, self-hosted personal finance.",
        theme_color: "#0A0F02",
        background_color: "#0A0F02",
        display: "standalone",
        start_url: "/",
        scope: "/",
      },
      workbox: {
        // Precache the built app shell. Navigation falls back to index.html
        // (SPA). Do NOT cache /api/* — those are live, auth'd, and event streams.
        globPatterns: ["**/*.{js,css,html,svg,woff2,png,ico}"],
        navigateFallback: "/index.html",
        navigateFallbackDenylist: [/^\/api\//],
        runtimeCaching: [],
      },
      devOptions: { enabled: false }, // never register the SW in `npm run dev`
    }),
  ],
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
