-- Custom Reports canvas (Actual-style widgets) — vertical stack, drag-handle reorder.
-- Each row is one widget. Per-user DB so no user_id column.
-- Seeded lazily on first list when empty to preserve current Reports as default.

CREATE TABLE report_widgets (
  id TEXT PRIMARY KEY,
  position INTEGER NOT NULL,
  title TEXT NOT NULL CHECK(length(title) > 0 AND length(title) <= 120),
  chart_type TEXT NOT NULL CHECK(chart_type IN ('table','bar','barStacked','line','area','donut')),
  split_by TEXT NOT NULL CHECK(split_by IN ('category','group','payee','account','month','spendingType')),
  period TEXT NOT NULL CHECK(period IN ('Last1Month','Last3Months','Last6Months','YTD','All')),
  filters_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_report_widgets_position ON report_widgets(position ASC, id ASC);
