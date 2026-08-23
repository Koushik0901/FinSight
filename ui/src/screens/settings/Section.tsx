import type { ReactNode } from "react";

export function Section({
  id,
  title,
  description,
  children,
}: {
  id: string;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section id={`sec-${id}`}>
      <h2 className="h1" style={{ fontSize: 26 }}>
        {title}
      </h2>
      <div className="muted" style={{ marginTop: 6 }}>
        {description}
      </div>
      <div style={{ marginTop: 18 }}>{children}</div>
    </section>
  );
}
