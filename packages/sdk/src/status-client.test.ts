import { StatusClient } from "./status-client";
import { NotReady } from "./status";
import { Clock } from "@sub-rosa/time";

// Mock fetch
const mockFetch = (response: Response | Promise<Response>) => {
  global.fetch = jest.fn(() => response);
};

// Fake clock
const createFakeClock = (now: number): Clock => ({
  now: () => now,
});

describe("StatusClient", () => {
  const contractId = "test-contract";
  const keeperUrl = "http://localhost:3000";
  const timeoutMs = 1000;

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe("getStatus", () => {
    it("returns live snapshot on successful response", async () => {
      const snapshot = {
        roundId: "1",
        contractId,
        data: { key: "value" },
      };
      mockFetch(
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve(snapshot),
        } as Response)
      );

      const client = new StatusClient({
        keeperUrl,
        contractId,
        timeoutMs,
      });

      const status = await client.getStatus();
      expect(status).toEqual(snapshot);
    });

    it("returns not-ready on timeout and drops previous snapshot", async () => {
      mockFetch(
        new Promise((_, reject) =>
          setTimeout(() => reject(new Error("Timeout")), timeoutMs + 100)
        )
      );

      const fakeClock = createFakeClock(0);
      const client = new StatusClient({
        keeperUrl,
        contractId,
        timeoutMs,
        clock: fakeClock,
      });

      // Simulate previous snapshot
      client["snapshot"] = {
        roundId: "0",
        contractId,
        data: { key: "old" },
      };

      const status = await client.getStatus();
      expect(status).toBeInstanceOf(NotReady);
      expect(client["snapshot"]).toBeUndefined();
    });

    it("returns not-ready on mismatched contractId", async () => {
      const snapshot = {
        roundId: "1",
        contractId: "wrong-contract",
        data: { key: "value" },
      };
      mockFetch(
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve(snapshot),
        } as Response)
      );

      const client = new StatusClient({
        keeperUrl,
        contractId,
        timeoutMs,
      });

      const status = await client.getStatus();
      expect(status).toBeInstanceOf(NotReady);
    });

    it("returns not-ready on invalid response body", async () => {
      mockFetch(
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ invalid: "body" }),
        } as Response)
      );

      const client = new StatusClient({
        keeperUrl,
        contractId,
        timeoutMs,
      });

      const status = await client.getStatus();
      expect(status).toBeInstanceOf(NotReady);
    });

    it("strips userinfo from keeper URL", async () => {
      const snapshot = {
        roundId: "1",
        contractId,
        data: { key: "value" },
      };
      mockFetch(
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve(snapshot),
        } as Response)
      );

      const client = new StatusClient({
        keeperUrl: "http://user:pass@localhost:3000",
        contractId,
        timeoutMs,
      });

      await client.getStatus();
      expect(fetch).toHaveBeenCalledWith(
        "http://localhost:3000/status",
        expect.anything()
      );
    });
  });
});
