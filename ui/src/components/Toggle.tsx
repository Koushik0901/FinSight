/**
 * The app's switch primitive — a bare styled span with `role="switch"`.
 *
 * Extracted from Settings so the notification-policy panel (and any future
 * surface) shares one keyboard/ARIA implementation instead of re-deriving it.
 * Keep it presentational: it owns no state and no business rules.
 */
export function Toggle({
  checked,
  onChange,
  ariaLabel,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  /** Accessible name. Required whenever the switch isn't labelled by adjacent text. */
  ariaLabel?: string;
}) {
  return (
    <span
      className={`tog${checked ? " on" : ""}`}
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      tabIndex={0}
      onClick={() => onChange(!checked)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onChange(!checked);
        }
      }}
    />
  );
}
