import { explain } from "./useMetrics";

test("explain returns pantry strings", () => {
  expect(explain("displayMedian")).toMatch(/Smooth/);
  expect(explain("recentMean90")).toMatch(/Recent/);
});

test("explain safetyConservative", () => {
  expect(explain("safetyConservative")).toMatch(/Conservative/);
});
