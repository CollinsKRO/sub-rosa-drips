export interface Snapshot {
  roundId: string;
  contractId: string;
  data: Record<string, unknown>;
}

export class NotReady {
  readonly ready = false;
}

export type Status = Snapshot | NotReady;
