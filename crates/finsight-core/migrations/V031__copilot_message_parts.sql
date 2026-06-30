-- Persist assistant-ui compatible rich message parts while preserving plain text.
ALTER TABLE conversation_messages ADD COLUMN parts_json TEXT;
