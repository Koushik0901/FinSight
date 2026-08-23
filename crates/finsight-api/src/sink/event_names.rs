/// One streamed chunk/frame of a Copilot answer (both chat runtimes).
pub const COPILOT_STREAM_FRAME: &str = "copilot-stream-frame";
/// A finished background Copilot answer (async follow-up delivery).
pub const COPILOT_ASYNC_ANSWER: &str = "copilot-async-answer";
/// CSV import row progress.
pub const IMPORT_PROGRESS: &str = "import-progress";
/// CSV import finished (success or failure).
pub const IMPORT_COMPLETE: &str = "import-complete";
/// Background AI categorizer made progress on its scan.
pub const CATEGORIZATION_PROGRESS: &str = "categorization.progress";
/// Background AI categorizer finished its scan.
pub const CATEGORIZATION_COMPLETE: &str = "categorization.complete";
/// Background agent job failed (no dedicated frontend listener yet; still
/// part of the wire contract via SSE).
pub const AGENT_ERROR: &str = "agent.error";
/// SSE comment-frame keep-alive sent by finsight-server.
pub const KEEP_ALIVE: &str = "finsight:keepalive";
/// Frontend-synthesized only (httpBackend on a 401, logout): never emitted
/// by the backend. Listed so the whole event vocabulary lives in one place.
pub const AUTH_REQUIRED: &str = "finsight:auth-required";

/// Every name above. Drives the generated TypeScript mirror and its
/// parity guard.
pub const ALL: &[&str] = &[
    COPILOT_STREAM_FRAME,
    COPILOT_ASYNC_ANSWER,
    IMPORT_PROGRESS,
    IMPORT_COMPLETE,
    CATEGORIZATION_PROGRESS,
    CATEGORIZATION_COMPLETE,
    AGENT_ERROR,
    KEEP_ALIVE,
    AUTH_REQUIRED,
];
