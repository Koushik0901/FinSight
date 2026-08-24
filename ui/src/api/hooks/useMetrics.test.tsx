import { explain } from "./useMetrics";
test("explain returns pantry strings", () => {
  expect(explain("displayMedian")).toMatch(/Smooth/);
  expect(explain("recentMean90")).toMatch(/Recent/);
});
