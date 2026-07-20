//! Deriving "go look at what just changed" links from executed agent actions.
//!
//! The approval flow used to end in prose — the user was told "3 actions
//! applied" and left in the chat with no way to see the result. This module
//! turns the actions that actually succeeded into an offer to visit the
//! screens they touched.
//!
//! The input is the executed action's **payload**, not the model's prose. That
//! matters: payloads carry real entity ids that the executor just wrote
//! against, so a link built here can never point at a screen that does not
//! exist or an entity the user does not have.
//!
//! Everything here is pure — no database, no IO — so the mapping is directly
//! testable against synthetic payloads.

use finsight_core::models::AgentNavigationTarget;
use finsight_core::routes::AppRoute;
use serde_json::Value;

/// The most screens we will offer at once. A bundle spanning more than a few
/// screens is better served by a short list than a wall of buttons.
const MAX_TARGETS: usize = 3;

/// One executed action, reduced to what navigation cares about.
#[derive(Debug, Clone, Copy)]
pub struct ExecutedAction<'a> {
    pub action_kind: &'a str,
    pub payload_json: &'a str,
}

/// The screen an action kind affects, plus the payload field holding the id of
/// the entity it touched (when the screen can focus one).
fn route_for_kind(action_kind: &str) -> Option<(AppRoute, Option<&'static str>)> {
    match action_kind {
        "set_budget" => Some((AppRoute::Budget, Some("categoryId"))),
        "update_goal_monthly" | "update_goal_target" => Some((AppRoute::Goals, Some("goalId"))),
        // The ledger has no per-row focus param, so these land on the screen.
        "set_transaction_category" | "set_transaction_flag" | "recategorize_bulk" => {
            Some((AppRoute::Transactions, None))
        }
        "create_rule" => Some((AppRoute::Rules, None)),
        "save_scenario" => Some((AppRoute::Scenarios, None)),
        "generate_report" => Some((AppRoute::Reports, None)),
        // The row id is generated at execution time and is not in the payload,
        // so we can only offer the screen itself.
        "create_planned_transaction" => Some((AppRoute::Recurring, None)),
        // Debts are accounts; a plan may span several, so no single focus.
        "debt_payoff_plan" => Some((AppRoute::Accounts, None)),
        // Unknown kinds (including any added later without updating this map)
        // simply get no link rather than a guessed one.
        _ => None,
    }
}

/// Human label for a screen, e.g. "View in Budget".
fn label_for(route: AppRoute) -> &'static str {
    match route {
        AppRoute::Budget => "View in Budget",
        AppRoute::Goals => "View in Goals",
        AppRoute::Accounts => "View in Accounts",
        AppRoute::Transactions => "View in Transactions",
        AppRoute::Recurring => "View in Recurring",
        AppRoute::Rules => "View in Rules",
        AppRoute::Scenarios => "View in Scenarios",
        AppRoute::Reports => "View in Reports",
        AppRoute::Categories => "View in Categories",
        AppRoute::Settings => "Open Settings",
    }
}

/// Pull a string id out of a payload, tolerating both camelCase (how payloads
/// are serialised) and snake_case (in case a producer hand-rolls one).
fn payload_id(payload_json: &str, camel_key: &str) -> Option<String> {
    let value: Value = serde_json::from_str(payload_json).ok()?;
    let snake_key = to_snake_case(camel_key);
    for key in [camel_key, snake_key.as_str()] {
        if let Some(found) = value.get(key).and_then(|v| v.as_str()) {
            let trimmed = found.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn to_snake_case(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 2);
    for ch in camel.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Navigation offers for a set of executed actions.
///
/// One entry per distinct screen, in first-touched order. A screen is focused
/// on a specific entity only when every action touching that screen touched
/// the *same* entity — focusing an arbitrary one of several would be a
/// coin-flip dressed up as intent.
///
/// Callers pass only the actions that actually succeeded; a link to a change
/// that failed to apply would be actively misleading.
pub fn navigation_targets(actions: &[ExecutedAction<'_>]) -> Vec<AgentNavigationTarget> {
    // (route, entity ids seen) in first-touched order.
    let mut ordered: Vec<(AppRoute, Vec<Option<String>>)> = Vec::new();

    for action in actions {
        let Some((route, id_key)) = route_for_kind(action.action_kind) else {
            continue;
        };
        let entity = id_key.and_then(|key| payload_id(action.payload_json, key));
        match ordered.iter_mut().find(|(r, _)| *r == route) {
            Some((_, seen)) => seen.push(entity),
            None => ordered.push((route, vec![entity])),
        }
    }

    ordered
        .into_iter()
        .take(MAX_TARGETS)
        .map(|(route, seen)| {
            // Focus only when the screen supports it and there is exactly one
            // distinct entity behind every action that hit this screen.
            let mut distinct: Vec<&String> = seen.iter().flatten().collect();
            distinct.sort();
            distinct.dedup();
            let path = match (distinct.as_slice(), seen.len()) {
                ([only], count) if count == seen.iter().filter(|e| e.is_some()).count() => {
                    route.focused(only)
                }
                _ => route.path().to_string(),
            };
            AgentNavigationTarget {
                label: label_for(route).to_string(),
                path,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::routes::is_known_route;

    fn action<'a>(kind: &'a str, payload: &'a str) -> ExecutedAction<'a> {
        ExecutedAction {
            action_kind: kind,
            payload_json: payload,
        }
    }

    #[test]
    fn a_budget_edit_links_to_that_category() {
        let targets = navigation_targets(&[action(
            "set_budget",
            r#"{"categoryId":"cat-groceries","month":"2026-07","amountCents":45000}"#,
        )]);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].label, "View in Budget");
        assert_eq!(targets[0].path, "/budget?focusCategory=cat-groceries");
    }

    #[test]
    fn a_goal_edit_links_to_that_goal() {
        let targets = navigation_targets(&[action(
            "update_goal_monthly",
            r#"{"goalId":"goal-ef","monthlyDeltaCents":10000}"#,
        )]);
        assert_eq!(targets[0].path, "/goals?focusGoal=goal-ef");
    }

    #[test]
    fn two_edits_to_the_same_category_still_focus_it() {
        let targets = navigation_targets(&[
            action(
                "set_budget",
                r#"{"categoryId":"cat-x","month":"2026-07","amountCents":1}"#,
            ),
            action(
                "set_budget",
                r#"{"categoryId":"cat-x","month":"2026-08","amountCents":2}"#,
            ),
        ]);
        assert_eq!(targets.len(), 1, "same screen should collapse to one offer");
        assert_eq!(targets[0].path, "/budget?focusCategory=cat-x");
    }

    #[test]
    fn edits_to_different_categories_link_to_the_screen_not_an_arbitrary_one() {
        let targets = navigation_targets(&[
            action(
                "set_budget",
                r#"{"categoryId":"cat-a","month":"2026-07","amountCents":1}"#,
            ),
            action(
                "set_budget",
                r#"{"categoryId":"cat-b","month":"2026-07","amountCents":2}"#,
            ),
        ]);
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].path, "/budget",
            "picking one of two touched categories would be arbitrary"
        );
    }

    #[test]
    fn a_bundle_spanning_screens_offers_each_in_first_touched_order() {
        let targets = navigation_targets(&[
            action(
                "update_goal_target",
                r#"{"goalId":"g-1","targetCents":500000}"#,
            ),
            action(
                "set_budget",
                r#"{"categoryId":"c-1","month":"2026-07","amountCents":1}"#,
            ),
        ]);
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].path, "/goals?focusGoal=g-1");
        assert_eq!(targets[1].path, "/budget?focusCategory=c-1");
    }

    #[test]
    fn offers_are_capped_so_the_ui_never_gets_a_wall_of_buttons() {
        let targets = navigation_targets(&[
            action("create_rule", r#"{"pattern":"X","categoryId":"c"}"#),
            action("save_scenario", r#"{"description":"d","params":{}}"#),
            action("generate_report", r#"{"reportType":"a","scope":"b"}"#),
            action("debt_payoff_plan", r#"{"method":"snowball"}"#),
        ]);
        assert_eq!(targets.len(), MAX_TARGETS);
    }

    #[test]
    fn screens_without_a_focus_param_link_bare() {
        let targets = navigation_targets(&[action(
            "set_transaction_category",
            r#"{"transactionId":"t-1","categoryId":"c-1"}"#,
        )]);
        assert_eq!(targets[0].path, "/transactions");
        assert_eq!(targets[0].label, "View in Transactions");
    }

    #[test]
    fn unknown_action_kinds_produce_no_link_rather_than_a_guess() {
        let targets = navigation_targets(&[action("teleport_funds", r#"{"anything":1}"#)]);
        assert!(targets.is_empty());
    }

    #[test]
    fn malformed_and_empty_payloads_degrade_to_the_bare_screen() {
        // Not JSON at all.
        let broken = navigation_targets(&[action("set_budget", "not json {{{")]);
        assert_eq!(broken[0].path, "/budget");

        // Valid JSON, but the id field is missing.
        let missing = navigation_targets(&[action("set_budget", r#"{"month":"2026-07"}"#)]);
        assert_eq!(missing[0].path, "/budget");

        // Present but blank — must not yield `?focusCategory=`.
        let blank = navigation_targets(&[action("set_budget", r#"{"categoryId":"   "}"#)]);
        assert_eq!(blank[0].path, "/budget");

        // Wrong type for the id.
        let wrong_type = navigation_targets(&[action("set_budget", r#"{"categoryId":42}"#)]);
        assert_eq!(wrong_type[0].path, "/budget");
    }

    #[test]
    fn snake_case_payload_keys_are_also_accepted() {
        let targets = navigation_targets(&[action(
            "set_budget",
            r#"{"category_id":"cat-snake","month":"2026-07"}"#,
        )]);
        assert_eq!(targets[0].path, "/budget?focusCategory=cat-snake");
    }

    #[test]
    fn no_actions_means_no_offer() {
        assert!(navigation_targets(&[]).is_empty());
    }

    #[test]
    fn ids_from_arbitrary_imported_data_stay_url_safe() {
        // Ids are not guaranteed to be UUIDs — they can come from whatever an
        // institution's export used. An id containing `&` must not forge a
        // second query parameter.
        let targets = navigation_targets(&[action(
            "set_budget",
            r#"{"categoryId":"a&admin=1","month":"2026-07"}"#,
        )]);
        assert_eq!(targets[0].path, "/budget?focusCategory=a%26admin%3D1");
    }

    #[test]
    fn every_generated_path_is_a_route_the_frontend_renders() {
        // The whole point of deriving from payloads: no dead links, ever.
        let all_kinds = [
            ("set_budget", r#"{"categoryId":"c"}"#),
            ("update_goal_monthly", r#"{"goalId":"g"}"#),
            ("update_goal_target", r#"{"goalId":"g"}"#),
            ("set_transaction_category", r#"{"transactionId":"t"}"#),
            ("set_transaction_flag", r#"{"transactionId":"t"}"#),
            ("recategorize_bulk", r#"{"assignments":[]}"#),
            ("create_rule", r#"{"pattern":"p"}"#),
            ("save_scenario", r#"{"description":"d"}"#),
            ("generate_report", r#"{"reportType":"r"}"#),
            ("create_planned_transaction", r#"{"description":"d"}"#),
            ("debt_payoff_plan", r#"{"method":"snowball"}"#),
        ];
        for (kind, payload) in all_kinds {
            for target in navigation_targets(&[action(kind, payload)]) {
                assert!(
                    is_known_route(&target.path),
                    "{kind} produced unroutable path {}",
                    target.path
                );
                assert!(!target.label.is_empty(), "{kind} produced an empty label");
            }
        }
    }

    #[test]
    fn every_executable_action_kind_has_a_navigation_mapping() {
        // Pins this map against `executor::execute_item`. A new action kind
        // added there without a mapping here silently loses its CTA.
        for kind in [
            "set_budget",
            "update_goal_monthly",
            "update_goal_target",
            "set_transaction_category",
            "set_transaction_flag",
            "create_rule",
            "save_scenario",
            "generate_report",
            "create_planned_transaction",
            "recategorize_bulk",
            "debt_payoff_plan",
        ] {
            assert!(
                route_for_kind(kind).is_some(),
                "executable action kind '{kind}' has no navigation mapping"
            );
        }
    }
}
