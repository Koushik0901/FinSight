/* Monoline icon set — 16px, stroke 1.4, currentColor
   Ported from the Plutus design prototype. */

import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

const icon =
  (children: React.ReactNode, viewBox = "0 0 16 16") =>
  ({ "aria-hidden": ariaHidden, ...props }: IconProps) => (
    <svg
      viewBox={viewBox}
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={ariaHidden ?? "true"}
      focusable="false"
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

export const Recipe = icon(
  <>
    <rect x="2.5" y="3" width="11" height="10.5" rx="1.8" />
    <path d="M5 1.8v2.4M11 1.8v2.4M2.5 6.2h11" />
    <path d="M5.2 9h3.2" />
    <path d="m9.8 8 1.8 1.8-1.8 1.8" />
  </>
);

export const Goal = icon(
  <>
    <circle cx="8" cy="8" r="6" />
    <circle cx="8" cy="8" r="3" />
    <circle cx="8" cy="8" r="0.8" fill="currentColor" stroke="none" />
  </>
);

export const Journey = icon(
  <>
    <circle cx="8" cy="8" r="5.5" />
    <path d="M8 5.2 9.4 8H6.6L8 5.2z" fill="currentColor" stroke="none" />
    <path d="M8 8v3" />
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

export const Info = icon(
  <>
    <circle cx="8" cy="8" r="6" />
    <path d="M8 7.2v3.6" />
    <circle cx="8" cy="5" r="0.7" fill="currentColor" stroke="none" />
  </>,
);

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

export const Brain = icon(
  <>
    <path d="M6 3.5C6 2.67 6.67 2 7.5 2S9 2.67 9 3.5" />
    <path d="M3.5 6.5C2.67 6.5 2 7.17 2 8s.67 1.5 1.5 1.5" />
    <path d="M12.5 6.5c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5" />
    <path d="M4 6a3 3 0 0 1 3-3h2a3 3 0 0 1 3 3v4a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3V6z" />
    <path d="M6 9h4M7 7h2" />
  </>
);

export const Send = icon(
  <path d="M2.5 8 13.5 3l-3 5 3 5L2.5 8zm5.5 0h5.5" />
);

export const Cpu = icon(
  <>
    <rect x="4.5" y="4.5" width="7" height="7" rx="1" />
    <path d="M7 4.5V3M9 4.5V3M7 13v-1.5M9 13v-1.5M4.5 7H3M4.5 9H3M13 7h-1.5M13 9h-1.5" />
  </>
);

export const House = icon(<path d="M2.5 8 8 3l5.5 5M4 7.5V13h8V7.5" />);

export const Cart = icon(
  <>
    <path d="M2.5 3h2l1 8h7M5.5 8h6.5" />
    <circle cx="6.5" cy="13" r="0.9" />
    <circle cx="11" cy="13" r="0.9" />
  </>
);

export const Fork = icon(
  <>
    <path d="M4 2v12M4 6h3a1 1 0 0 1 1 1v4" />
    <path d="M12 2v12M12 6V2" />
  </>
);

export const Car = icon(
  <>
    <path d="M2.5 11V8l1.5-3h8l1.5 3v3" />
    <path d="M2.5 11h11M3.5 13a1 1 0 1 0 0-2M12.5 13a1 1 0 1 0 0-2" />
  </>
);

export const Bulb = icon(
  <>
    <path d="M5.5 10.5a4 4 0 1 1 5 0V12H5.5z" />
    <path d="M6.5 14h3" />
  </>
);

export const Box = icon(
  <>
    <path d="M2.5 5.5 8 3l5.5 2.5L8 8z" />
    <path d="M2.5 5.5V11L8 13.5V8M13.5 5.5V11L8 13.5" />
  </>
);

export const Heart = icon(
  <path d="M8 13s-5-3.2-5-7a2.5 2.5 0 0 1 5-.5 2.5 2.5 0 0 1 5 .5c0 3.8-5 7-5 7z" />
);

export const Plane = icon(<path d="m2 9 12-5-3 11-3-4.5z" />);

export const Gift = icon(
  <>
    <rect x="2.5" y="6" width="11" height="3" rx="0.6" />
    <rect x="3.5" y="9" width="9" height="5" rx="0.6" />
    <path d="M8 6v8M5.5 6c0-1.5 1-2.5 2.5-2.5S10.5 4.5 10.5 6" />
  </>
);
