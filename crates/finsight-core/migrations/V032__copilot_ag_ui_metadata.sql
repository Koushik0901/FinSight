ALTER TABLE conversation_messages
    ADD COLUMN run_status TEXT NOT NULL DEFAULT 'completed'
    CHECK(run_status IN ('streaming', 'completed', 'cancelled', 'errored', 'requires_action'));

ALTER TABLE conversation_messages
    ADD COLUMN ag_ui_metadata_json TEXT;
