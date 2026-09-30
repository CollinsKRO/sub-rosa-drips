import { Clock } from "@sub-rosa/time";
import { Status, NotReady, Snapshot } from "./status";

interface StatusClientConfig {
  keeperUrl: string;
  contractId: string;
  timeoutMs: number;
  clock?: Clock;
}

export class StatusClient {
  private readonly keeperUrl: string;
  private readonly contractId: string;
  private readonly timeoutMs: number;
  private readonly clock: Clock;
  private snapshot?: Snapshot;

  constructor(config: StatusClientConfig) {
    this.keeperUrl = new URL(config.keeperUrl).toString(); // Normalize and strip userinfo
    this.contractId = config.contractId;
    this.timeoutMs = config.timeoutMs;
    this.clock = config.clock ?? { now: () => Date.now() };
  }

  async getStatus(): Promise<Status> {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      const response = await fetch(`${this.keeperUrl}/status`, {
        signal: controller.signal,
        headers: { "Content-Type": "application/json" },
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        return new NotReady();
      }

      const body: unknown = await response.json();
      const snapshot = this.parseSnapshot(body);

      if (!this.isValidSnapshot(snapshot)) {
        return new NotReady();
      }

      this.snapshot = snapshot;
      return snapshot;
    } catch (error) {
      clearTimeout(timeoutId);
      if (error instanceof Error && error.name === "AbortError") {
        this.snapshot = undefined; // Drop previous snapshot on timeout
        return new NotReady();
      }
      this.snapshot = undefined;
      return new NotReady();
    }
  }

  private parseSnapshot(body: unknown): Snapshot | null {
    if (
      typeof body === "object" &&
      body !== null &&
      "roundId" in body &&
      "contractId" in body &&
      "data" in body
    ) {
      return {
        roundId: String((body as Record<string, unknown>).roundId),
        contractId: String((body as Record<string, unknown>).contractId),
        data: (body as Record<string, unknown>).data as Record<string, unknown>,
      };
    }
    return null;
  }

  private isValidSnapshot(snapshot: Snapshot | null): snapshot is Snapshot {
    if (!snapshot) return false;
    return (
      snapshot.contractId === this.contractId &&
      snapshot.roundId !== undefined
    );
  }
}
