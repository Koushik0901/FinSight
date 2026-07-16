use std::collections::{BTreeMap, BTreeSet};

/// Parse `bindings.ts` into cmd → set(camelCase arg keys). Matches the two shapes
/// tauri-specta emits: `TAURI_INVOKE("cmd")` and `TAURI_INVOKE("cmd", { a, b })`.
fn parse_bindings() -> BTreeMap<String, BTreeSet<String>> {
    let src = include_str!("../../../ui/src/api/bindings.ts");
    let mut out = BTreeMap::new();
    for chunk in src.split("TAURI_INVOKE(\"").skip(1) {
        let cmd = chunk.split('"').next().unwrap().to_string();
        // Args object (if any) is the `{ ... }` before the first `)` after the name.
        let head = &chunk[..chunk.find(')').unwrap_or(chunk.len())];
        let mut keys = BTreeSet::new();
        if let (Some(o), Some(c)) = (head.find('{'), head.rfind('}')) {
            for raw in head[o + 1..c].split(',') {
                // shorthand `{ id, balanceCents }` OR `{ id: id }` — take the key.
                let k = raw.split(':').next().unwrap().trim();
                if !k.is_empty() {
                    keys.insert(k.to_string());
                }
            }
        }
        out.insert(cmd, keys);
    }
    out
}

/// Parse `dispatch.rs` into cmd → set(keys read via `arg(&p, "key")`) by walking
/// each `"cmd" =>` match arm up to the next arm.
fn parse_dispatch_arg_keys() -> BTreeMap<String, BTreeSet<String>> {
    let src = include_str!("../src/dispatch.rs");
    // Everything after `match cmd {` (skip the const arrays above it).
    let body = &src[src.find("match cmd {").expect("match cmd block")..];
    let mut out = BTreeMap::new();
    // Arm headers look like:  "list_accounts" =>
    let arm_re = regex::Regex::new(r#""([a-z0-9_]+)"\s*=>"#).unwrap();
    let key_re = regex::Regex::new(r#"arg\(&p,\s*"([A-Za-z0-9_]+)"\)"#).unwrap();
    let arms: Vec<(usize, String)> = arm_re
        .captures_iter(body)
        .map(|c| (c.get(0).unwrap().start(), c[1].to_string()))
        .collect();
    for (i, (start, cmd)) in arms.iter().enumerate() {
        let end = arms.get(i + 1).map(|(s, _)| *s).unwrap_or(body.len());
        let mut keys = BTreeSet::new();
        for k in key_re.captures_iter(&body[*start..end]) {
            keys.insert(k[1].to_string());
        }
        out.insert(cmd.clone(), keys);
    }
    out
}

#[test]
fn every_binding_command_is_routed_or_explicitly_unsupported() {
    let wanted: BTreeSet<String> = parse_bindings().keys().cloned().collect();
    assert!(wanted.len() > 100, "bindings parse looks broken: {}", wanted.len());
    let routed: BTreeSet<String> = finsight_server::dispatch::SUPPORTED
        .iter()
        .chain(finsight_server::dispatch::UNSUPPORTED)
        .map(|s| s.to_string())
        .collect();
    let missing: Vec<_> = wanted.difference(&routed).collect();
    let stale: Vec<_> = routed.difference(&wanted).collect();
    assert!(missing.is_empty(), "bindings.ts commands with no server route: {missing:?}");
    assert!(stale.is_empty(), "server routes for commands not in bindings.ts: {stale:?}");
}

/// THE arg-key guard: for every SUPPORTED command (minus exemptions), the keys the
/// dispatcher reads via `arg(&p, "…")` must EXACTLY equal the keys bindings.ts sends.
/// Catches `balance_cents` vs `balanceCents`, missing args, and typos — at test time.
#[test]
fn dispatcher_arg_keys_match_bindings_exactly() {
    let bindings = parse_bindings();
    let dispatch = parse_dispatch_arg_keys();
    let exempt: BTreeSet<&str> = finsight_server::dispatch::ARG_CHECK_EXEMPT
        .iter()
        .copied()
        .collect();
    let mut problems = Vec::new();
    for cmd in finsight_server::dispatch::SUPPORTED {
        if exempt.contains(cmd) {
            continue;
        }
        let want = bindings
            .get(*cmd)
            .unwrap_or_else(|| panic!("SUPPORTED command `{cmd}` absent from bindings.ts"));
        let got = dispatch.get(*cmd).cloned().unwrap_or_default();
        if &got != want {
            problems.push(format!(
                "  {cmd}: bindings sends {want:?} but dispatcher reads {got:?}"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "dispatcher arg-key mismatches (fix the arg(&p, \"…\") keys):\n{}",
        problems.join("\n")
    );
}
