-- Cross-account transfer pairing: links the withdrawal leg in one account to
-- the matching deposit leg in another (internal transfer, credit-card payment).
-- NULL = unpaired. Reciprocal: if A.transfer_peer_id = B then B.transfer_peer_id = A.
-- ON DELETE SET NULL so deleting one leg leaves the survivor unpaired, never dangling.
ALTER TABLE transactions ADD COLUMN transfer_peer_id TEXT REFERENCES transactions(id) ON DELETE SET NULL;
