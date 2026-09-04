import { useEffect, useRef, useState, type ReactNode } from "react";
import { useReducedMotion } from "../utils/plotMotion";

type Props = {
  children: ReactNode;
  /** Delay before the reveal animation starts (ms) — use for stagger inside a list */
  delay?: number;
  /** How much of the element must be visible before it reveals (0-1) */
  threshold?: number;
  /** Extra root margin, e.g. "0px 0px -80px 0px" to start a bit early */
  rootMargin?: string;
  /** If true, re-hide when scrolled out (default false — reveal once) */
  once?: boolean;
  className?: string;
};

/**
 * Reveal on scroll — wraps any plot/card and only runs its
 * `.plot-*` animations when it enters the viewport.
 * Uses IntersectionObserver, respects `prefers-reduced-motion`.
 */
export default function Reveal({
  children,
  delay = 0,
  threshold = 0.18,
  rootMargin = "0px 0px -10% 0px",
  once = true,
  className = "",
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const reduced = useReducedMotion();

  useEffect(() => {
    if (reduced) {
      setVisible(true);
      return;
    }
    const el = ref.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setVisible(true);
          if (once) obs.disconnect();
        } else if (!once) {
          setVisible(false);
        }
      },
      { threshold, rootMargin },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [reduced, threshold, rootMargin, once]);

  // Reduced motion: no wrapper needed, show immediately with no animation
  if (reduced) return <>{children}</>;

  return (
    <div
      ref={ref}
      className={`reveal ${visible ? "is-visible" : ""} ${className}`.trim()}
      style={delay ? ({ ["--reveal-delay" as string]: `${delay}ms` } as React.CSSProperties) : undefined}
    >
      {children}
    </div>
  );
}
