import { describe, expect, it } from "vitest";
import { formatCalendarDate, parseCalendarDate } from "./date";

describe("calendar-date helpers", () => {
  it("keeps date-only values on their entered local calendar day", () => {
    const date = parseCalendarDate("2027-08-01");
    expect([date.getFullYear(), date.getMonth(), date.getDate()]).toEqual([2027, 7, 1]);
    expect(formatCalendarDate("2027-08-01", { month: "short", year: "numeric" })).toBe("Aug 2027");
  });

  it("preserves timestamp semantics", () => {
    expect(parseCalendarDate("2027-08-01T12:34:56Z").toISOString()).toBe("2027-08-01T12:34:56.000Z");
  });
});
