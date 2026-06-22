-- V020: planning metadata for account liquidity and income cadence
ALTER TABLE accounts ADD COLUMN liquidity_type TEXT NOT NULL DEFAULT 'liquid';
ALTER TABLE accounts ADD COLUMN emergency_fund_eligible INTEGER NOT NULL DEFAULT 1;
ALTER TABLE accounts ADD COLUMN goal_earmark TEXT;

INSERT OR IGNORE INTO settings(key, value) VALUES('planning.paycheck_cadence', 'null');
INSERT OR IGNORE INTO settings(key, value) VALUES('planning.expected_paycheck_cents', 'null');