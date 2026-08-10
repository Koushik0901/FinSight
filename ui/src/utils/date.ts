/**
 * Parse an ISO calendar date without letting JavaScript reinterpret it as UTC.
 *
 * `new Date("2027-08-01")` is midnight UTC, which is still July 31 in much of
 * North America. Values from `<input type="date">` and date-only API fields are
 * calendar dates, not instants, so they must be constructed from local fields.
 * Timestamps continue through the normal Date parser.
 */
export function parseCalendarDate(value: string): Date {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return new Date(value);
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}

export function formatCalendarDate(
  value: string,
  options: Intl.DateTimeFormatOptions,
  locale = "en-US",
): string {
  return parseCalendarDate(value).toLocaleDateString(locale, options);
}
