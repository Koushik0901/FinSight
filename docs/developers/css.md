# CSS & Design Conventions

## Tokens

All UI color, spacing, radius, and shadow values live in `ui/src/styles/tokens.css`. Do not hardcode them in components.

| Token | Value | Notes |
|---|---|---|
| `--accent` | `#C9F950` | Vivid lime — reserved for the single most important action |
| `--accent-ink` | `#0A0F02` | Text on accent |
| `--accent-soft` / `--accent-line` | `rgba(201,249,80,…)` | Calm tint for routine surfaces / borders |
| `--surface` / `--surface-2` / `--elevated` | Theme-dependent | Card surfaces |
| `--line` / `--line-2` | Theme-dependent | Hairlines — keep them thin |
| `--ink` / `--ink-mute` / `--ink-faint` | Theme-dependent | Text |
| `--radius` / `--radius-lg` | `10px` / `14px` | Card radii |
| `--sans` / `--mono` | Geist Variable | Font stacks |

Dark (`data-theme="dark"`) is primary: `--bg: #08080B`, `--surface: #101015`. Light: `--bg: #F8F7F2`, `--surface: #FFFFFF`.

## Utilities

`ui/src/styles/app.css` defines shared classes. Prefer them over one-off styles:

- `.card` `.chip` `.btn` `.tbl` `.stat` `.eyebrow` `.toolbar` `.stream` `.goal-bar`
- `.money` — amounts that must blur in Privacy mode (`[data-privacy="on"] .money`)

## Conventions

- Use tokens, not hex literals.
- Reuse components (`ui/src/components/`) and the utility classes; do not invent a second card style.
- Privacy-mode amounts must carry `"money"` — CSS blurs them automatically.
- Motion: use expo easings (`--ease-out-expo`, `--ease-out-quart`), never bounce/elastic; respect `prefers-reduced-motion`.
- Density via `data-density="cozy"|"compact"` and `pointer: coarse` for touch targets.

## Docs theme

The VitePress theme at `docs/.vitepress/theme/` reuses the FinSight tokens as `--fs-*` and maps them to VitePress brand vars, so docs and app share the same accent and the same calm.

See also: [Frontend](/developers/frontend), [Privacy & Tokens](/getting-started/privacy).
