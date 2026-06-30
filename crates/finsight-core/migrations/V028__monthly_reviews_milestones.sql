-- V028: monthly reviews + net worth milestones

CREATE TABLE monthly_reviews (
    id TEXT PRIMARY KEY,
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    notes TEXT,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(year, month)
);

CREATE INDEX idx_monthly_reviews_year_month ON monthly_reviews(year DESC, month DESC);

CREATE TABLE net_worth_milestones (
    threshold_cents INTEGER PRIMARY KEY,
    achieved_at TEXT NOT NULL
);
