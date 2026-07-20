//! Canonical registry of the frontend's navigable routes.
//!
//! Several backend surfaces hand the UI a path to navigate to: Inbox action
//! items, Copilot post-execution CTAs, and missing-data prompts. Two of those
//! are Rust-authored and trustworthy; the Copilot's `action_path` is authored
//! by the *model*, which means it can invent a screen that does not exist and
//! hand the user a dead link.
//!
//! This module is the one place that knows which paths are real. Rust-authored
//! producers build paths through [`AppRoute`] so they cannot typo one, and
//! model-authored paths are filtered through [`is_known_route`] before they
//! ever reach the UI.
//!
//! Keeping this in `finsight-core` (rather than in the API or agent crate) is
//! deliberate: both `finsight-agent` (executor navigation) and `finsight-api`
//! (answer validation, inbox items) need it, and neither depends on the other.
//!
//! **Mirrored in `ui/src/routes.ts`.** Both sides carry a test that pins the
//! list, so a route added to `App.tsx` without updating this file fails the
//! frontend suite, and vice versa.

/// Every path the frontend router will render, excluding parameterised
/// segments (see [`is_known_route`] for how those are matched).
///
/// Order matches the `<Routes>` block in `ui/src/App.tsx`.
pub const APP_ROUTES: &[&str] = &[
    "/",
    "/inbox",
    "/import-review",
    "/insights",
    "/accounts",
    "/transactions",
    "/budget",
    "/categories",
    "/recurring",
    "/goals",
    "/journey",
    "/scenarios",
    "/reports",
    "/path-back",
    "/rules",
    "/settings",
    "/settings/users",
    "/copilot",
    "/recipes",
];

/// A screen the backend can point the user at, with the query parameter it
/// uses to focus a specific entity.
///
/// The `focus*` idiom (read the param on mount, scroll/highlight the row, then
/// strip the param from the URL) is already established by the Accounts, Goals
/// and Recurring screens; new entries should follow it rather than inventing a
/// second convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppRoute {
    Budget,
    Goals,
    Accounts,
    Transactions,
    Recurring,
    Rules,
    Scenarios,
    Reports,
    Categories,
    Settings,
}

impl AppRoute {
    /// The bare path, with no query string.
    pub fn path(self) -> &'static str {
        match self {
            AppRoute::Budget => "/budget",
            AppRoute::Goals => "/goals",
            AppRoute::Accounts => "/accounts",
            AppRoute::Transactions => "/transactions",
            AppRoute::Recurring => "/recurring",
            AppRoute::Rules => "/rules",
            AppRoute::Scenarios => "/scenarios",
            AppRoute::Reports => "/reports",
            AppRoute::Categories => "/categories",
            AppRoute::Settings => "/settings",
        }
    }

    /// The query parameter this screen reads to focus one entity, if it has
    /// one. Screens without a focus param return `None` and are linked bare.
    /// Only screens that genuinely read the param are listed. Emitting a
    /// `?focusFoo=` that no screen consumes would be a dead link parameter —
    /// it looks like it works and silently does nothing.
    pub fn focus_param(self) -> Option<&'static str> {
        match self {
            AppRoute::Budget => Some("focusCategory"),
            AppRoute::Goals => Some("focusGoal"),
            AppRoute::Accounts => Some("focusAccount"),
            AppRoute::Recurring => Some("focusPlanned"),
            // The transactions ledger deep-links by `?filter=`, not by row id;
            // there is no per-transaction focus to target.
            AppRoute::Transactions
            | AppRoute::Rules
            | AppRoute::Scenarios
            | AppRoute::Reports
            | AppRoute::Categories
            | AppRoute::Settings => None,
        }
    }

    /// Build a link to this screen focused on `entity_id`.
    ///
    /// Falls back to the bare path when the screen has no focus param or the
    /// id is blank — a link to the right screen beats no link at all, and it
    /// can never produce `?focusGoal=` with an empty value.
    pub fn focused(self, entity_id: &str) -> String {
        let trimmed = entity_id.trim();
        match (self.focus_param(), trimmed.is_empty()) {
            (Some(param), false) => {
                format!("{}?{}={}", self.path(), param, encode_query_value(trimmed))
            }
            _ => self.path().to_string(),
        }
    }
}

/// Percent-encode the characters that would otherwise break out of a query
/// value. Entity ids are UUID-shaped today, but ids can come from imported
/// data, so this must not assume that.
fn encode_query_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Whether `path` is a route the frontend will actually render.
///
/// Accepts an optional query string and/or fragment, and matches the one
/// parameterised route (`/accounts/:id/transactions`) structurally. Used to
/// filter model-authored paths: a hallucinated `/networth` is dropped rather
/// than rendered as a CTA that dead-ends on the fallback route.
pub fn is_known_route(path: &str) -> bool {
    let trimmed = path.trim();
    if !trimmed.starts_with('/') {
        return false;
    }
    // Strip fragment first, then query — a fragment may contain '?'.
    let without_fragment = trimmed.split('#').next().unwrap_or("");
    let bare = without_fragment.split('?').next().unwrap_or("");
    // Tolerate a trailing slash on anything but the root route itself.
    let bare = if bare.len() > 1 {
        bare.trim_end_matches('/')
    } else {
        bare
    };

    if APP_ROUTES.contains(&bare) {
        return true;
    }

    // `/accounts/:id/transactions` — the only parameterised route.
    let segments: Vec<&str> = bare.split('/').filter(|s| !s.is_empty()).collect();
    matches!(segments.as_slice(), ["accounts", id, "transactions"] if !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_registry_is_well_formed() {
        for route in APP_ROUTES {
            assert!(route.starts_with('/'), "route {route} must be absolute");
            assert!(
                !route.ends_with('/') || *route == "/",
                "route {route} must not have a trailing slash"
            );
            assert!(
                !route.contains(':'),
                "route {route} must not be parameterised"
            );
        }
        let mut seen = APP_ROUTES.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), APP_ROUTES.len(), "duplicate route in APP_ROUTES");
    }

    /// Pins this registry against its TypeScript mirror.
    ///
    /// The frontend cannot check this direction: reading `crates/` from a
    /// Vitest file means escaping the Vite root. So Rust owns the
    /// cross-language half, and `ui/src/routes.test.ts` owns the
    /// `routes.ts` ↔ `App.tsx` half. Together they make a route that exists in
    /// only two of the three places a test failure rather than a dead link.
    #[test]
    fn ts_mirror_matches_the_rust_registry() {
        let mirror = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../ui/src/routes.ts")
            .canonicalize()
            .expect("ui/src/routes.ts should exist alongside this crate");
        let source = std::fs::read_to_string(&mirror).expect("could not read ui/src/routes.ts");

        // Take only the APP_ROUTES literal — NON_LINKABLE_ROUTES follows it.
        let start = source
            .find("export const APP_ROUTES = [")
            .expect("APP_ROUTES not found in routes.ts");
        let body = &source[start..];
        let end = body.find(']').expect("unterminated APP_ROUTES array");

        let ts_routes: Vec<String> = body[..end]
            .split('"')
            .skip(1)
            .step_by(2)
            .map(|s| s.to_string())
            .collect();

        assert_eq!(
            ts_routes,
            APP_ROUTES.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
            "ui/src/routes.ts and routes.rs have diverged — a backend link to a \
             route the frontend does not render is a dead end, so update both"
        );
    }

    #[test]
    fn known_routes_are_accepted_with_and_without_query() {
        assert!(is_known_route("/"));
        assert!(is_known_route("/budget"));
        assert!(is_known_route("/budget?focusCategory=abc"));
        assert!(is_known_route("/settings/users"));
        assert!(is_known_route("/budget/"));
        assert!(is_known_route("  /goals  "));
        assert!(is_known_route("/transactions?filter=no_category#top"));
    }

    #[test]
    fn parameterised_account_route_matches_structurally() {
        assert!(is_known_route("/accounts/abc-123/transactions"));
        assert!(is_known_route("/accounts/any-id-at-all/transactions"));
        // Missing or extra segments are not the route.
        assert!(!is_known_route("/accounts//transactions"));
        assert!(!is_known_route("/accounts/abc/transactions/extra"));
        assert!(!is_known_route("/accounts/abc"));
    }

    #[test]
    fn unknown_and_malformed_paths_are_rejected() {
        // The failure this whole module exists to prevent: a model inventing
        // a plausible-sounding screen.
        assert!(!is_known_route("/networth"));
        assert!(!is_known_route("/debts"));
        assert!(!is_known_route("/budgets")); // note the plural — real route is /budget
        assert!(!is_known_route(""));
        assert!(!is_known_route("budget")); // relative
        assert!(!is_known_route("https://evil.example.com/budget"));
        assert!(!is_known_route("//evil.example.com"));
    }

    #[test]
    fn focused_links_use_each_screens_own_param() {
        assert_eq!(AppRoute::Budget.focused("cat-1"), "/budget?focusCategory=cat-1");
        assert_eq!(AppRoute::Goals.focused("g-1"), "/goals?focusGoal=g-1");
        assert_eq!(AppRoute::Accounts.focused("a-1"), "/accounts?focusAccount=a-1");
        assert_eq!(AppRoute::Recurring.focused("p-1"), "/recurring?focusPlanned=p-1");
    }

    #[test]
    fn focused_degrades_to_the_bare_screen_when_it_cannot_do_better() {
        // No focus param on this screen.
        assert_eq!(AppRoute::Rules.focused("r-1"), "/rules");
        // Blank id must never yield a dangling `?focusGoal=`.
        assert_eq!(AppRoute::Goals.focused(""), "/goals");
        assert_eq!(AppRoute::Goals.focused("   "), "/goals");
    }

    #[test]
    fn focused_links_encode_ids_that_are_not_url_safe() {
        // Ids originate from imported data, so they are not guaranteed to be
        // UUIDs. An id containing `&` must not forge a second query param.
        let link = AppRoute::Budget.focused("a&b=c");
        assert_eq!(link, "/budget?focusCategory=a%26b%3Dc");
        assert!(is_known_route(&link));

        // Non-ASCII ids survive as valid percent-encoded UTF-8.
        let unicode = AppRoute::Goals.focused("café");
        assert_eq!(unicode, "/goals?focusGoal=caf%C3%A9");
        assert!(is_known_route(&unicode));
    }

    #[test]
    fn every_focused_link_is_itself_a_known_route() {
        // Guards against a focus param being added to a screen that was never
        // registered in APP_ROUTES.
        for route in [
            AppRoute::Budget,
            AppRoute::Goals,
            AppRoute::Accounts,
            AppRoute::Transactions,
            AppRoute::Recurring,
            AppRoute::Rules,
            AppRoute::Scenarios,
            AppRoute::Reports,
            AppRoute::Categories,
            AppRoute::Settings,
        ] {
            assert!(
                is_known_route(route.path()),
                "{} is not in APP_ROUTES",
                route.path()
            );
            assert!(
                is_known_route(&route.focused("sample-id")),
                "focused link for {} is not a known route",
                route.path()
            );
        }
    }
}
