/* Monoline icon set — 16px, stroke 1.4, currentColor
   Ported from the Plutus design prototype. */

import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

const icon =
  (children: React.ReactNode, viewBox = "0 0 16 16") =>
  (props: IconProps) => (
    <svg
      viewBox={viewBox}
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      {children}
    </svg>
  );

export const Today = icon(
  <>
    <circle cx="8" cy="8" r="5.5" />
    <path d="M8 4.5v3.5l2.2 1.4" />
  </>
);

export const Wallet = icon(
  <>
    <rect x="2" y="4" width="12" height="9" rx="1.5" />
    <path d="M2 7h12" />
    <circle cx="11.5" cy="10" r="0.6" fill="currentColor" stroke="none" />
  </>
);

export const Flow = icon(
  <>
    <path d="M2 11.5 5 8l2.5 2.5L13 4" />
    <path d="M9.5 4H13v3.5" />
  </>
);

export const Grid = icon(
  <>
    <rect x="2" y="2" width="5" height="5" rx="0.6" />
    <rect x="9" y="2" width="5" height="5" rx="0.6" />
    <rect x="2" y="9" width="5" height="5" rx="0.6" />
    <rect x="9" y="9" width="5" height="5" rx="0.6" />
  </>
);

export const Repeat = icon(
  <>
    <path d="M3 6.5V5a1.5 1.5 0 0 1 1.5-1.5h7L13 5l-1.5 1.5" />
    <path d="M13 9.5V11a1.5 1.5 0 0 1-1.5 1.5h-7L3 11l1.5-1.5" />
  </>
);

export const Goal = icon(
  <>
    <circle cx="8" cy="8" r="6" />
    <circle cx="8" cy="8" r="3" />
    <circle cx="8" cy="8" r="0.8" fill="currentColor" stroke="none" />
  </>
);

export const Lego = icon(
  <>
    <path d="M3 7h10v6H3z" />
    <path d="M5 7V4.5h2V7M9 7V4.5h2V7" />
  </>
);

export const Gear = icon(
  <>
    <circle cx="8" cy="8" r="2" />
    <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.5 3.5l1.4 1.4M11.1 11.1l1.4 1.4M3.5 12.5l1.4-1.4M11.1 4.9l1.4-1.4" />
  </>
);

export const Sparkle = icon(
  <path d="M8 2v3M8 11v3M2 8h3M11 8h3M4 4l2 2M10 10l2 2M4 12l2-2M10 6l2-2" />
);

export const Search = icon(
  <>
    <circle cx="7" cy="7" r="4.5" />
    <path d="m10.5 10.5 3 3" />
  </>
);

export const ArrowRight = icon(<path d="M3 8h10M9 4l4 4-4 4" />);
export const ArrowLeft = icon(<path d="M13 8H3M7 4 3 8l4 4" />);
export const ArrowDown = icon(<path d="M8 3v10M4 9l4 4 4-4" />);
export const ArrowUp = icon(<path d="M8 13V3M4 7l4-4 4 4" />);

export const Plus = icon(<path d="M8 3v10M3 8h10" />);
export const Check = icon(<path d="m3 8 3.5 3.5L13 5" />);
export const X = icon(<path d="m4 4 8 8M12 4l-8 8" />);

export const Lock = icon(
  <>
    <rect x="3" y="7" width="10" height="7" rx="1.5" />
    <path d="M5 7V5a3 3 0 0 1 6 0v2" />
  </>
);

export const Eye = icon(
  <>
    <path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" />
    <circle cx="8" cy="8" r="1.8" />
  </>
);

export const EyeOff = icon(
  <>
    <path d="M3 3l10 10" />
    <path d="M6 6.2C3.8 7.4 1.5 8 1.5 8s2.5 4.5 6.5 4.5c1.1 0 2.1-.3 3-.7" />
    <path d="M9.6 4.1A6.7 6.7 0 0 1 14.5 8s-.7 1.3-2 2.5" />
  </>
);

export const Filter = icon(<path d="M2 4h12L9.5 9v4L6.5 14V9z" />);

export const Bolt = icon(<path d="m9 1.5-6 7.5h4l-1 5.5 6-7.5H8z" />);

export const Bell = icon(
  <>
    <path d="M4 11V8a4 4 0 0 1 8 0v3l1 1.5H3z" />
    <path d="M6.5 13a1.5 1.5 0 0 0 3 0" />
  </>
);

export const Spark = icon(<path d="M2 11l3-4 2.5 2.5L11 5l3 4" />);

export const More = icon(
  <>
    <circle cx="3.5" cy="8" r="1" fill="currentColor" stroke="none" />
    <circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
    <circle cx="12.5" cy="8" r="1" fill="currentColor" stroke="none" />
  </>
);

export const Pencil = icon(
  <>
    <path d="m3 13 1-3 7-7 2 2-7 7z" />
    <path d="m9 5 2 2" />
  </>
);

export const Trash = icon(
  <>
    <path d="M3 4.5h10M6 4.5V3a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.5" />
    <path d="M4.5 4.5 5 13.2c0 .4.4.8.8.8h4.4c.4 0 .8-.4.8-.8L11.5 4.5" />
    <path d="M7 7v5M9 7v5" />
  </>
);

export const Tag = icon(
  <>
    <path d="M3 3h5.5L13 7.5 8.5 12 4 7.5z" />
    <circle cx="6" cy="6" r="0.8" fill="currentColor" stroke="none" />
  </>
);

export const Down = icon(<path d="m4 6 4 4 4-4" />);
export const Up = icon(<path d="m4 10 4-4 4 4" />);
