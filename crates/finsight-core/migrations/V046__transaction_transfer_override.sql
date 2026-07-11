-- User verdict on whether a transaction is a transfer between their own
-- accounts. NULL = no verdict (automatic detection applies), 1 = user says
-- transfer, 0 = user says real income/spending. The categorizer and the
-- transfer-pairing pass must never overwrite a non-NULL verdict.
ALTER TABLE transactions ADD COLUMN transfer_override INTEGER NULL;
