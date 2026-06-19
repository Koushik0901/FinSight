import type { ReactNode, SelectHTMLAttributes } from "react";

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: ReactNode;
  error?: string | null;
  hint?: ReactNode;
  children: ReactNode;
}

export default function Select({ label, error, hint, children, className = "", id, ...rest }: SelectProps) {
  const selectId = id ?? (typeof label === "string" ? label.toLowerCase().replace(/\s+/g, "-") : undefined);
  const hasError = !!error;

  return (
    <label className={`field ${className}`.trim()} htmlFor={selectId}>
      {label}
      <select id={selectId} className={hasError ? "err" : undefined} aria-invalid={hasError} aria-describedby={hasError ? `${selectId}-error` : undefined} {...rest}>
        {children}
      </select>
      {hasError && (
        <span id={`${selectId}-error`} className="err" role="alert">
          {error}
        </span>
      )}
      {hint && !hasError && <span className="muted" style={{ fontSize: 12 }}>{hint}</span>}
    </label>
  );
}
