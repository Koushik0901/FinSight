-- Backfill the conscious-spending bucket (Ramit Sethi: fixed / investments /
-- savings / guilt_free) for the starter categories. Nothing ever auto-assigned
-- spending_type, so the Budget "Spending mix" showed everything as untagged.
-- Mirrors `default_spending_type` in crates/finsight-core/src/categorize.rs.
-- Guarded on NULL so a user-chosen tag is never overwritten.
UPDATE categories SET spending_type = 'fixed'      WHERE id = 'housing'       AND spending_type IS NULL;
UPDATE categories SET spending_type = 'fixed'      WHERE id = 'utilities'     AND spending_type IS NULL;
UPDATE categories SET spending_type = 'fixed'      WHERE id = 'subscriptions' AND spending_type IS NULL;
UPDATE categories SET spending_type = 'fixed'      WHERE id = 'groceries'     AND spending_type IS NULL;
UPDATE categories SET spending_type = 'fixed'      WHERE id = 'transport'     AND spending_type IS NULL;
UPDATE categories SET spending_type = 'fixed'      WHERE id = 'health'        AND spending_type IS NULL;
UPDATE categories SET spending_type = 'guilt_free' WHERE id = 'dining'        AND spending_type IS NULL;
UPDATE categories SET spending_type = 'guilt_free' WHERE id = 'shopping'      AND spending_type IS NULL;
UPDATE categories SET spending_type = 'guilt_free' WHERE id = 'travel'        AND spending_type IS NULL;
UPDATE categories SET spending_type = 'guilt_free' WHERE id = 'gifts'         AND spending_type IS NULL;
