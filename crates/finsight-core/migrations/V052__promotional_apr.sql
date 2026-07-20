-- Promotional APR expiry on credit accounts.
--
-- A card on a 0% balance-transfer promo that ends in three months was stored
-- identically to one permanently at 0%: a single `apr_pct`. Debt-payoff ranking
-- sorts on that number, so it confidently ranked a soon-to-be-22.99% balance
-- last and the Copilot stated that ordering as sound advice. A promo expiry is
-- also one of the few genuinely time-critical events in personal finance — the
-- warning is only useful BEFORE it lands.
--
-- `apr_pct` deliberately KEEPS its meaning: the rate in effect right now. That
-- is what makes this backward compatible — every existing row stays correct
-- with no data rewrite, every existing consumer keeps reading the rate that
-- applies today, and an account with no promo behaves exactly as it did. The
-- new columns describe only what changes LATER.
--
-- Both nullable, matching the other optional debt columns from V039. NULL
-- `promo_apr_expires_on` means "no promotional period", which is the correct
-- default for every account that already exists.

ALTER TABLE accounts ADD COLUMN promo_apr_expires_on TEXT;
ALTER TABLE accounts ADD COLUMN post_promo_apr_pct REAL;
