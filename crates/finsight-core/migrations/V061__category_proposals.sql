-- Issue #87 (Slice 1): proposal + provenance foundation for categorization.
--
-- Records each automated categorization SUGGESTION as its own first-class
-- row, decoupled from the canonical `transactions.category_id` write it may
-- or may not have made (see `applied`). This slice is purely ADDITIVE: the
-- LLM pass still auto-writes canonical exactly as it did before this table
-- existed, AND records a proposal here (`applied = 1`). A future ML pass
-- (not built in this issue) could insert a proposal with `applied = 0` — a
-- suggestion with no canonical write, gated on a precision benchmark that
-- doesn't exist yet — using this same table without another migration.
--
-- `status` and `applied` are deliberately separate columns: `status` tracks
-- the review lifecycle (pending -> accepted / corrected / rejected) and
-- `applied` tracks whether THIS proposal's category was written to
-- `transactions.category_id` at proposal time. A single status enum can't
-- carry both axes without conflating "what a human decided" with "did we
-- act automatically" — see docs on `crates/finsight-core/src/models/category_proposal.rs`.
--
-- `txn_id` is UNIQUE: this table holds the CURRENT outstanding suggestion
-- per transaction, not a full history of every categorization attempt (the
-- append-only `categorizations` table, V003, already is that history). A new
-- automated suggestion for a transaction supersedes whatever proposal row
-- was there before (upsert on txn_id), mirroring how `transactions.ai_confidence`
-- is a single live column rather than a log. If a future issue needs
-- per-attempt proposal history, that's an additive change (e.g. a companion
-- log table), not a breaking one — nothing here assumes only one proposal
-- ever exists for a transaction, only that at most one is "live" at a time.
CREATE TABLE category_proposals (
  id                    TEXT PRIMARY KEY,
  txn_id                TEXT NOT NULL UNIQUE REFERENCES transactions(id) ON DELETE CASCADE,
  proposed_category_id  TEXT NOT NULL REFERENCES categories(id),
  source                TEXT NOT NULL,            -- 'llm' today; reserved: 'ml' for a future pass
  confidence            REAL NOT NULL,
  rationale             TEXT,
  candidates_json       TEXT,                     -- ranked candidate categories as a JSON array; NULL until a multi-candidate pass exists
  status                TEXT NOT NULL DEFAULT 'pending'
                          CHECK (status IN ('pending', 'accepted', 'corrected', 'rejected')),
  applied               INTEGER NOT NULL,          -- 1 = this proposal's category was written to transactions.category_id when the row was created
  model                 TEXT,
  created_at            TEXT NOT NULL,
  reviewed_at           TEXT                       -- stamped when a human resolves the proposal (accept/correct/reject); NULL = still pending or auto-accepted without review
);
CREATE INDEX idx_category_proposals_status ON category_proposals(status);
