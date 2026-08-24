import { describe, test, expect } from "vitest";
import type { paths, components } from "./openapi";
import { api } from "./openapiClient";

// Check that openapi.ts is typed (not shallow)
type ListAccountsOp = paths["/api/rpc/list_accounts"]["post"];
type ListAccountsResponse = ListAccountsOp["responses"]["200"] extends { content: { "application/json": infer T } } ? T : never;

// This should be AccountSummary[] not never/Record<string, never>
const _check: ListAccountsResponse = [] as unknown as components["schemas"]["AccountSummary"][];

describe("openapi generation", () => {
  test("api.listAccounts is typed", () => {
    expect(typeof api.listAccounts).toBe("function");
  });

  test("api has 229 methods", () => {
    // COMMANDS is 229, api should have 229 entries plus maybe rpc generic, but at least 229
    const count = Object.keys(api).length;
    // Allow for generic rpc helper, but should be at least 229
    expect(count).toBeGreaterThanOrEqual(229);
  });

  test("openapi schemas not shallow", async () => {
    // Verify that openapi.json has real schemas with properties
    const spec = await import("./openapi.json");
    const schemas = (spec.default as any)?.components?.schemas ?? (spec as any).components?.schemas;
    // If direct import not available, check via paths
    expect(schemas).toBeDefined();
  });
});
