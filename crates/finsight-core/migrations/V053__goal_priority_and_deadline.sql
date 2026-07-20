-- Goal priority and deadline strictness.
--
-- Goals were reasoned about as independent line items. There was no way to say
-- that the emergency fund matters more than the vacation fund, or that a
-- wedding date is immovable while "new laptop" is aspirational — so every
-- multi-goal question (allocation, conflicts, what to fund first) had to guess.
-- `run_goal_allocation_scenarios` inferred an order from goal type and date
-- precisely because the user had no way to state one.
--
-- PRIORITY is a small enum rather than an integer rank. A rank has to be
-- renumbered on every insert, delete, and reorder, and there is no defensible
-- rank to give a brand-new goal. An enum needs no maintenance and every goal
-- has an obvious default.
--
-- It is deliberately SEPARATE from `sort_order`, which already exists and means
-- something different: sort_order is where the user dragged the card, priority
-- is how much the goal matters. Overloading one for the other would make
-- reordering a list silently change financial advice.
--
-- DEADLINE STRICTNESS says what the existing `target_date` actually means. The
-- same date can be a hard commitment (a wedding, a visa fee, a tax bill) or a
-- hope (paying off the car "by summer"), and allocation should treat those
-- differently. Note a goal with no `target_date` is inherently open-ended
-- whatever this column says — the domain layer resolves that rather than
-- trusting the column alone.
--
-- Both default to the neutral value, so every existing goal keeps behaving
-- exactly as it does today and no data is rewritten.

ALTER TABLE goals ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal';
-- 'critical' | 'high' | 'normal' | 'someday'

ALTER TABLE goals ADD COLUMN deadline_strictness TEXT NOT NULL DEFAULT 'target';
-- 'hard' | 'target' | 'none'
