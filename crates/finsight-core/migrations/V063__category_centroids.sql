-- Issue #92 (Slice 4b): prototype/centroid embedding per category.
--
-- One vector per category — the mean of its curated examples (V062) in the
-- encoder's embedding space. Categorizing a transaction is then a linear
-- cosine scan over ~tens of these vectors, NOT a nearest-neighbour search over
-- the whole ledger. That distinction is the entire reason this slice needs no
-- ANN/vector-index infrastructure; see the scoping note on epic #74.
--
-- WHY A SIDE TABLE, NOT A BLOB COLUMN ON `categories`
-- Two reasons, both about the vector being meaningless on its own:
--   1. A vector is only interpretable next to the `model_id` that produced it.
--      Swapping encoders invalidates every vector at once, and that is one
--      `DELETE FROM category_centroids` here versus an UPDATE touching every
--      row of a table the whole app reads.
--   2. `categories` is a small, hot, user-facing table. A ~1.5KB BLOB per row
--      (384 f32s for MiniLM) would ride along in every `SELECT *` the app does
--      for a list nobody is asking embeddings for.
--
-- INVALIDATION IS THE POINT OF `model_id` AND `dims`
-- These are not bookkeeping. Comparing a vector from model A against a query
-- vector from model B does not error and does not return an obviously wrong
-- answer — it returns a PLAUSIBLE one, silently, forever. That is the exact
-- failure shape this codebase keeps getting bitten by, so the read path must
-- check both and SKIP a mismatch rather than score it. Storing them per row
-- (not once globally) also means a partially-migrated table degrades to
-- "fewer categories match" instead of "every match is garbage".
CREATE TABLE category_centroids (
  -- PK, not just FK: exactly one centroid per category. ON DELETE CASCADE so a
  -- hard-deleted category cannot leave a vector behind that still matches.
  --
  -- ARCHIVING is deliberately NOT handled here. `categories::archive` is a soft
  -- delete (it only stamps `archived_at`), so the row survives and the centroid
  -- with it — matching how V062 examples and V034 guidance already behave. The
  -- read path filters `archived_at IS NULL`, which is what stops an archived
  -- category from matching; keeping the vector means un-archiving does not
  -- require a re-embed (and re-embedding costs a model load).
  category_id    TEXT PRIMARY KEY REFERENCES categories(id) ON DELETE CASCADE,

  -- The encoder that produced `vector`, e.g. "sentence-transformers/all-MiniLM-L6-v2".
  model_id       TEXT NOT NULL,
  -- Vector length. Redundant with `length(vector)/4` on purpose: an explicit
  -- column makes a truncated/corrupt BLOB detectable rather than reinterpreted
  -- as a shorter valid vector.
  dims           INTEGER NOT NULL,
  -- `dims` little-endian f32s. Stored L2-NORMALIZED, so cosine similarity is a
  -- plain dot product at read time and the read path never has to re-derive a
  -- norm it cannot verify.
  vector         BLOB NOT NULL,

  -- How many examples went into the mean. Diagnostic, and the honest input to
  -- any future confidence weighting: a centroid from one example is a point,
  -- not a prototype, and callers deserve to be able to tell.
  example_count  INTEGER NOT NULL,
  updated_at     TEXT NOT NULL
);

-- The read path scans "every centroid for the current model", so the model
-- filter is the selective one, not the category.
CREATE INDEX idx_category_centroids_model ON category_centroids(model_id);
