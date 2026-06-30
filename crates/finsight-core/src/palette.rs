//! Canonical color mapping for starter categories.
//! Mirrors `--c-*` tokens in `ui/src/styles/tokens.css` and the
//! `paletteFor` helper in `ui/src/utils/categoryColor.ts`.
//! If you change one side, change the other.

pub const DEFAULT_COLOR: &str = "#94A3B8";

/// `(id, hex)` pairs for the canonical 10 starter categories plus the
/// `subs` alias used by the walking-skeleton seed path.
pub const PALETTE: &[(&str, &str)] = &[
    ("housing", "#A78BFA"),
    ("groceries", "#34D399"),
    ("dining", "#FB923C"),
    ("transport", "#60A5FA"),
    ("utilities", "#FACC15"),
    ("subscriptions", "#F472B6"),
    ("subs", "#F472B6"),
    ("health", "#2DD4BF"),
    ("shopping", "#FCA5A5"),
    ("travel", "#818CF8"),
    ("gifts", "#FDE68A"),
];

pub fn color_for(id: &str) -> &'static str {
    PALETTE
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, c)| *c)
        .unwrap_or(DEFAULT_COLOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_ids_resolve_to_their_palette_color() {
        assert_eq!(color_for("housing"), "#A78BFA");
        assert_eq!(color_for("groceries"), "#34D399");
        assert_eq!(color_for("dining"), "#FB923C");
        assert_eq!(color_for("transport"), "#60A5FA");
        assert_eq!(color_for("utilities"), "#FACC15");
        assert_eq!(color_for("subscriptions"), "#F472B6");
        assert_eq!(color_for("subs"), "#F472B6");
        assert_eq!(color_for("health"), "#2DD4BF");
        assert_eq!(color_for("shopping"), "#FCA5A5");
        assert_eq!(color_for("travel"), "#818CF8");
        assert_eq!(color_for("gifts"), "#FDE68A");
    }

    #[test]
    fn unknown_id_falls_back_to_default_grey() {
        assert_eq!(color_for("not-a-category"), DEFAULT_COLOR);
        assert_eq!(color_for(""), DEFAULT_COLOR);
    }

    #[test]
    fn palette_entries_are_well_formed_hex() {
        assert!(!PALETTE.is_empty(), "palette must not be empty");
        for (id, color) in PALETTE {
            assert!(
                color.len() == 7
                    && color.starts_with('#')
                    && color[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "color for {id} must be #RRGGBB, got {color}"
            );
        }
    }

    #[test]
    fn palette_keys_are_unique() {
        let mut keys: Vec<&str> = PALETTE.iter().map(|(k, _)| *k).collect();
        keys.sort();
        let original_len = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), original_len, "palette has duplicate keys");
    }
}
