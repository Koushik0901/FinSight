-- Backfill canonical category colors.
-- Three seed paths (`sample.rs`, `seed_dev_demo`, `commit_starter_categories`)
-- previously stamped '#94A3B8' (slate grey) for every category, leaving the
-- UI unable to color-code. This migration restores the per-category palette
-- defined in `crates/finsight-core/src/palette.rs` and mirrored in
-- `ui/src/utils/categoryColor.ts`.
UPDATE categories SET color = '#A78BFA' WHERE id = 'housing';
UPDATE categories SET color = '#34D399' WHERE id = 'groceries';
UPDATE categories SET color = '#FB923C' WHERE id = 'dining';
UPDATE categories SET color = '#60A5FA' WHERE id = 'transport';
UPDATE categories SET color = '#FACC15' WHERE id = 'utilities';
UPDATE categories SET color = '#F472B6' WHERE id = 'subscriptions';
UPDATE categories SET color = '#2DD4BF' WHERE id = 'health';
UPDATE categories SET color = '#FCA5A5' WHERE id = 'shopping';
UPDATE categories SET color = '#818CF8' WHERE id = 'travel';
UPDATE categories SET color = '#FDE68A' WHERE id = 'gifts';
