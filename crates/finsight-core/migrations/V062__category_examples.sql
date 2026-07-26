-- Issue #91 (Slice 4a): per-category example storage, keyed by the category's
-- STABLE id.
--
-- Until now the only per-category user signal was `categories.guidance` (V034),
-- a single free-text blob. The categorizer's so-called "examples" are the
-- GLOBAL last-5 user corrections (`load_recent_examples` in the agent crate) —
-- not scoped to a category at all. This table adds the missing per-category
-- exemplar set that #92 will embed into a prototype/centroid vector.
--
-- WHY TEXT, NOT A TRANSACTION REFERENCE
-- `example_text` is the load-bearing column and it is a denormalized SNAPSHOT,
-- not a lookup. A pure `txn_id` reference would silently lose examples: a
-- factory reset wipes `transactions` with foreign keys disabled, and a CSV
-- re-import churns transaction ids, so the exemplar set a user curated over
-- months would evaporate on the next re-import. What #92 consumes is the
-- description STRING (it embeds text, not amounts), so storing the string is
-- both the durable and the directly-useful choice.
--
-- `source_txn_id` is kept as an OPTIONAL provenance breadcrumb so an
-- "add this transaction as an example" affordance can point back at what the
-- user clicked. It is `ON DELETE SET NULL`, never CASCADE: losing the
-- transaction must degrade the example to "hand-typed", not delete it.
--
-- ARCHIVE BEHAVIOUR — hidden-but-retained, matching `guidance` exactly.
-- `categories::archive` only stamps `archived_at`; it has never cleared
-- `guidance`. The active-only consumer query (`guidance_hints`) is what
-- filters `archived_at IS NULL`. Examples follow the same split: rows survive
-- archiving (still listable by category id), and the active-only accessor
-- excludes them. So archiving orphans nothing, and #92's "prune or mark stale
-- centroids on archive" has a retained source to recompute from if the
-- category ever comes back.
--
-- The `categories(id)` reference IS `ON DELETE CASCADE` — unlike archive, a
-- hard delete of a category means the examples have no owner and no meaning.
CREATE TABLE category_examples (
  id             TEXT PRIMARY KEY,
  -- Keyed by the STABLE category id, never the label: renaming a category
  -- touches `categories.label` only, so examples ride through a rename.
  category_id    TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  example_text   TEXT NOT NULL,
  -- Provenance only. NULL = hand-typed, or the source transaction is gone.
  source_txn_id  TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  created_at     TEXT NOT NULL
);

-- One exemplar per (category, text). A duplicated example would double-weight
-- that point in #92's centroid mean, quietly skewing the prototype toward
-- whatever the user happened to add twice.
CREATE UNIQUE INDEX idx_category_examples_unique ON category_examples(category_id, example_text);
CREATE INDEX idx_category_examples_category ON category_examples(category_id);
