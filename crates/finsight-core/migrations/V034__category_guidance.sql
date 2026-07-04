-- Free-text guidance the user attaches to a category so the LLM categorizer and
-- Copilot know when it should be used (merchant hints, exclusions, intent).
ALTER TABLE categories ADD COLUMN guidance TEXT;
