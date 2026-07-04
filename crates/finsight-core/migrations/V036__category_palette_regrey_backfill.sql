-- Re-run the canonical palette backfill from V030 for the default categories.
-- `ensure_default_categories` (import-first seeding) kept stamping '#94A3B8'
-- grey AFTER V030 had already run, so DBs seeded through that path carry grey
-- for every category. The seeder now stamps palette colors; this fixes DBs
-- created in between. Guarded on the grey sentinel so a user-chosen color is
-- never overwritten.
UPDATE categories SET color = '#A78BFA' WHERE id = 'housing'       AND color = '#94A3B8';
UPDATE categories SET color = '#34D399' WHERE id = 'groceries'     AND color = '#94A3B8';
UPDATE categories SET color = '#FB923C' WHERE id = 'dining'        AND color = '#94A3B8';
UPDATE categories SET color = '#60A5FA' WHERE id = 'transport'     AND color = '#94A3B8';
UPDATE categories SET color = '#FACC15' WHERE id = 'utilities'     AND color = '#94A3B8';
UPDATE categories SET color = '#F472B6' WHERE id = 'subscriptions' AND color = '#94A3B8';
UPDATE categories SET color = '#2DD4BF' WHERE id = 'health'        AND color = '#94A3B8';
UPDATE categories SET color = '#FCA5A5' WHERE id = 'shopping'      AND color = '#94A3B8';
UPDATE categories SET color = '#818CF8' WHERE id = 'travel'        AND color = '#94A3B8';
UPDATE categories SET color = '#FDE68A' WHERE id = 'gifts'         AND color = '#94A3B8';
