import { afterEach, describe, expect, it, vi } from "vitest";
import { createUser, deleteUser, fetchAuthStatus, isServerMode, listUsers, login, logout, setup } from "./auth";

type AnyRec = Record<string, unknown>;

describe("auth.ts — server-mode auth API client", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    delete (window as unknown as AnyRec).__FINSIGHT_HTTP__;
  });

  describe("isServerMode", () => {
    it("is false when __FINSIGHT_HTTP__ is not set", () => {
      expect(isServerMode()).toBe(false);
    });

    it("is true when the httpBackend shim has installed __FINSIGHT_HTTP__", () => {
      (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
      expect(isServerMode()).toBe(true);
    });
  });

  describe("fetchAuthStatus", () => {
    it("GETs /api/auth/status and returns the parsed status", async () => {
      const fetchMock = vi.fn(async () =>
        new Response(
          JSON.stringify({ needsSetup: false, authenticated: true, username: "koushik", isAdmin: true }),
          { status: 200 }
        )
      );
      vi.stubGlobal("fetch", fetchMock);

      const status = await fetchAuthStatus();

      expect(fetchMock).toHaveBeenCalledWith("/api/auth/status");
      expect(status).toEqual({ needsSetup: false, authenticated: true, username: "koushik", isAdmin: true });
    });
  });

  describe("setup", () => {
    it("POSTs username/password and returns the recovery key", async () => {
      const fetchMock = vi.fn(async () =>
        new Response(JSON.stringify({ recoveryKey: "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-11111111-22222222" }), {
          status: 200,
        })
      );
      vi.stubGlobal("fetch", fetchMock);

      const result = await setup("koushik", "hunter2");

      expect(fetchMock).toHaveBeenCalledWith(
        "/api/auth/setup",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({ "content-type": "application/json" }),
          body: JSON.stringify({ username: "koushik", password: "hunter2" }),
        })
      );
      expect(result.recoveryKey.split("-")).toHaveLength(8);
    });

    it("throws the plain AppError object on 409 auth.already_setup", async () => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () =>
          new Response(JSON.stringify({ code: "auth.already_setup", message: "Setup already completed." }), {
            status: 409,
          })
        )
      );

      await expect(setup("koushik", "hunter2")).rejects.toEqual({
        code: "auth.already_setup",
        message: "Setup already completed.",
      });
    });
  });

  describe("login", () => {
    it("POSTs credentials and resolves on 200", async () => {
      const fetchMock = vi.fn(async () => new Response(JSON.stringify({}), { status: 200 }));
      vi.stubGlobal("fetch", fetchMock);

      await expect(login("koushik", "hunter2")).resolves.toBeUndefined();
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/auth/login",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ username: "koushik", password: "hunter2" }),
        })
      );
    });

    it("throws {code: 'auth.bad_credentials'} on 401", async () => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () =>
          new Response(JSON.stringify({ code: "auth.bad_credentials", message: "Wrong username or password." }), {
            status: 401,
          })
        )
      );

      await expect(login("koushik", "wrong")).rejects.toEqual({
        code: "auth.bad_credentials",
        message: "Wrong username or password.",
      });
    });
  });

  describe("logout", () => {
    it("POSTs to /api/auth/logout and resolves on 200", async () => {
      const fetchMock = vi.fn(async () => new Response(JSON.stringify({}), { status: 200 }));
      vi.stubGlobal("fetch", fetchMock);

      await expect(logout()).resolves.toBeUndefined();
      expect(fetchMock).toHaveBeenCalledWith("/api/auth/logout", expect.objectContaining({ method: "POST" }));
    });
  });

  describe("listUsers", () => {
    it("GETs /api/auth/users and returns the parsed list", async () => {
      const users = [
        { id: "u1", username: "koushik", isAdmin: true, createdAt: "2026-07-01T00:00:00Z" },
        { id: "u2", username: "sam", isAdmin: false, createdAt: "2026-07-02T00:00:00Z" },
      ];
      const fetchMock = vi.fn(async () => new Response(JSON.stringify(users), { status: 200 }));
      vi.stubGlobal("fetch", fetchMock);

      const result = await listUsers();

      expect(fetchMock).toHaveBeenCalledWith("/api/auth/users");
      expect(result).toEqual(users);
    });

    it("throws {code: 'auth.admin_required'} on 403", async () => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () =>
          new Response(JSON.stringify({ code: "auth.admin_required", message: "Admin access required." }), {
            status: 403,
          })
        )
      );

      await expect(listUsers()).rejects.toEqual({
        code: "auth.admin_required",
        message: "Admin access required.",
      });
    });
  });

  describe("createUser", () => {
    it("POSTs username/password and returns the recovery key", async () => {
      const fetchMock = vi.fn(async () =>
        new Response(JSON.stringify({ recoveryKey: "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-11111111-22222222" }), {
          status: 200,
        })
      );
      vi.stubGlobal("fetch", fetchMock);

      const result = await createUser("sam", "hunter2hunter2");

      expect(fetchMock).toHaveBeenCalledWith(
        "/api/auth/users",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({ "content-type": "application/json" }),
          body: JSON.stringify({ username: "sam", password: "hunter2hunter2" }),
        })
      );
      expect(result.recoveryKey.split("-")).toHaveLength(8);
    });
  });

  describe("deleteUser", () => {
    it("DELETEs /api/auth/users/{id} and resolves on 200", async () => {
      const fetchMock = vi.fn(async () => new Response(JSON.stringify({}), { status: 200 }));
      vi.stubGlobal("fetch", fetchMock);

      await expect(deleteUser("u2")).resolves.toBeUndefined();
      expect(fetchMock).toHaveBeenCalledWith("/api/auth/users/u2", expect.objectContaining({ method: "DELETE" }));
    });

    it("throws {code: 'auth.cannot_delete_self'} on 400", async () => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () =>
          new Response(JSON.stringify({ code: "auth.cannot_delete_self", message: "cannot delete your own account" }), {
            status: 400,
          })
        )
      );

      await expect(deleteUser("u1")).rejects.toEqual({
        code: "auth.cannot_delete_self",
        message: "cannot delete your own account",
      });
    });
  });
});
