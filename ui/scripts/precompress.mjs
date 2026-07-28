/**
 * Writes a `.br` and a `.gz` sibling next to every compressible file in
 * `dist/`, so `finsight-server` can serve pre-squeezed bytes instead of
 * compressing the same unchanging asset on every single request
 * (`ServeDir::precompressed_br()` picks them up automatically, and falls back
 * to the live `CompressionLayer` for anything missing).
 *
 * Why at build time: brotli at quality 11 is ~100x slower to compress than at
 * the default quality a server can afford per-request, but decompresses just
 * as fast. Paying that once here buys every user the smallest possible
 * download. This is the one place where the expensive setting is free.
 *
 * Uses only `node:zlib` — no dependency to add, audit, or keep current.
 */
import { constants, brotliCompressSync, gzipSync } from "node:zlib";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, extname } from "node:path";
import { fileURLToPath } from "node:url";

// fileURLToPath rather than `.pathname`: on Windows the latter yields a
// leading-slash `/E:/...`, and on any platform it leaves percent-encoding in
// place, so a checkout under a path containing a space would silently fail to
// resolve.
const DIST = fileURLToPath(new URL("../dist/", import.meta.url));

// Text formats only. woff2/png/ico are already compressed containers — running
// them through brotli spends CPU to produce a *larger* file, and a `.br` that
// loses to the original is worse than none (the server would serve it).
const COMPRESSIBLE = new Set([".js", ".css", ".html", ".svg", ".json", ".webmanifest", ".map"]);

// Below roughly one TCP segment, the response is a single packet either way, so
// compressing buys nothing and just adds files to the directory.
const MIN_BYTES = 1024;

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(full);
    else yield full;
  }
}

let files = 0;
let rawTotal = 0;
let brTotal = 0;

for (const file of walk(DIST)) {
  const ext = extname(file);
  if (!COMPRESSIBLE.has(ext)) continue;
  const raw = readFileSync(file);
  if (raw.length < MIN_BYTES) continue;

  const br = brotliCompressSync(raw, {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
      [constants.BROTLI_PARAM_SIZE_HINT]: raw.length,
    },
  });
  const gz = gzipSync(raw, { level: constants.Z_BEST_COMPRESSION });

  // Only keep a variant that actually wins. A `.br` bigger than the original
  // would be served in preference to it, making the site slower.
  if (br.length < raw.length) writeFileSync(`${file}.br`, br);
  if (gz.length < raw.length) writeFileSync(`${file}.gz`, gz);

  files += 1;
  rawTotal += raw.length;
  brTotal += Math.min(br.length, raw.length);
}

const kib = (n) => `${(n / 1024).toFixed(1)} KiB`;
const saved = rawTotal === 0 ? 0 : Math.round((1 - brTotal / rawTotal) * 100);
console.log(
  `precompress  ${files} files  ${kib(rawTotal)} → ${kib(brTotal)} brotli (−${saved}%)`
);

// A dist/ with no compressible files means the build didn't produce what we
// think it did; fail loudly rather than silently shipping uncompressed.
if (files === 0) {
  console.error("precompress: no compressible files found in dist/ — did the build run?");
  process.exit(1);
}
