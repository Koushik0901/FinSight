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
        // Register FinSight in the OS share sheet so a bank's CSV export can be
        // sent straight here from Files/Drive/an email attachment, instead of
        // the user having to find the file again from inside the app.
        //
        // The POST is answered by the service worker, NOT the server — see
        // public/share-target-sw.js for why (SameSite=Lax withholds the session
        // cookie from a cross-site POST navigation).
        share_target: {
          action: "/share-target",
          method: "POST",
          enctype: "multipart/form-data",
          params: {
            // `name: "file"` matches the field the SW reads AND the field
            // /api/import/csv already expects.
            files: [
              {
                name: "file",
                // Android in particular is inconsistent about the MIME type it
                // reports for a .csv — list the aliases it actually sends, plus
                // the extension, or the share sheet omits FinSight entirely.
                accept: [
                  "text/csv",
                  "text/comma-separated-values",
                  "application/csv",
                  "application/vnd.ms-excel",
                  ".csv",
                ],
              },
            ],
          },
        },
      },
      workbox: {
        // Precache the built app shell. Navigation falls back to index.html
        // (SPA). Do NOT cache /api/* — those are live, auth'd, and event streams.
        globPatterns: ["**/*.{js,css,html,svg,woff2,png,ico}"],
        navigateFallback: "/index.html",
        navigateFallbackDenylist: [/^\/api\//],
        runtimeCaching: [],
        // Extra handlers pulled into the generated Workbox worker, rather than
        // switching to injectManifest and hand-maintaining precache + SPA
        // fallback + the /api denylist above. These files live in public/ and
        // must keep existing: importScripts throws if one 404s, and a throwing
        // service worker fails to install, which would take offline support
        // down with it. Their presence and payload contracts are pinned by the
        // "service worker contract" block in src/pwa/shareTarget.test.ts, which
        // imports the file with `?raw`.
        importScripts: ["share-target-sw.js"],
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
