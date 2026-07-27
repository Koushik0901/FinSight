# FinSight component patterns

FinSight grows its shared UI incrementally. Extract a pattern only after it has
the same intent in at least three places; keep one-off financial visualizations
and context-specific compositions close to their screen.

## PageHeader

Use `ui/src/components/PageHeader.tsx` once at the top of a product screen. It
owns the page eyebrow, single `h1`, optional plain-language description, and
screen-level actions.

```tsx
<PageHeader
  eyebrow={<>Reports · {scopeLabel}</>}
  title="How money is moving."
  description="See the shape of your money over time."
  actions={<Button variant="outline">Export</Button>}
/>
```

- The default variant matches the compact product-screen heading.
- Use `variant="ruled"` when the heading needs a divider before the body.
- The lime signal dot is on by default; use `dot={false}` for neutral utility
  pages such as Settings.
- Do not use `PageHeader` for sections nested inside a screen. Preserve their
  local heading level and composition.

Spacing, type, and color come from `tokens.css` through the semantic classes in
`app.css`; avoid reintroducing inline heading styles in screens.
