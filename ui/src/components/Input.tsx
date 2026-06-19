import type { InputHTMLAttributes, ReactNode } from "react";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: ReactNode;
  error?: string | null;
  hint?: ReactNode;
}

export default function Input({ label, error, hint, className = "", id, ...rest }: InputProps) {
  const inputId = id ?? (typeof label === "string" ? label.toLowerCase().replace(/\s+/g, "-") : undefined);
  const hasError = !!error;

  return (
    <label className={`field ${className}`.trim()} htmlFor={inputId}>
      {label}
      <input id={inputId} className={hasError ? "err" : undefined} aria-invalid={hasError} aria-describedby={hasError ? `${inputId}-error` : undefined} {...rest} />
      {hasError && (
        <span id={`${inputId}-error`} className="err" role="alert">
          {error}
        </span>
      )}
      {hint && !hasError && <span className="muted" style={{ fontSize: 12 }}>{hint}</span>}
    </label>
  );
}
