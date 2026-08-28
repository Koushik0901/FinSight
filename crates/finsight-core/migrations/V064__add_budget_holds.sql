-- Hold for Next Month (Actual's Hold primitive) — park this month's unassigned
-- money for next month. Deducts from `to_budget` this month and appears as
-- income-like available next month. See docs/superpowers/plans/2026-08-23-what-to-steal-from-actual.md Task 2.
CREATE TABLE budget_holds (
  month TEXT PRIMARY KEY,
  amount_cents INTEGER NOT NULL CHECK(amount_cents >= 0)
);
