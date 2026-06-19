import type { TextareaHTMLAttributes, ReactNode } from "react";

interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: ReactNode;
  error?: string | null;
  hint?: ReactNode;
}

export default function TextArea({ label, error, hint, className = "", id, ...rest }: TextAreaProps) {
  const textAreaId = id ?? (typeof label === "string" ? label.toLowerCase().replace(/\s+/g, "-") : undefined);
  const hasError = !!error;

  return (
    <label className={`field ${className}`.trim()} htmlFor={textAreaId}>
      {label}
      <textarea id={textAreaId} className={hasError ? "err" : undefined} aria-invalid={hasError} aria-describedby={hasError ? `${textAreaId}-error` : undefined} {...rest} />
      {hasError && (
        <span id={`${textAreaId}-error`} className="err" role="alert">
          {error}
        </span>
      )}
      {hint && !hasError && <span className="muted" style={{ fontSize: 12 }}>{hint}</span>}
    </label>
  );
}
