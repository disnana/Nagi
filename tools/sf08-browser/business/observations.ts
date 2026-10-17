import { constants } from "node:fs";
import { open, type FileHandle } from "node:fs/promises";

export type FixtureKind = "login" | "cors";
export type OriginClass = "same_origin" | "allowed" | "denied";
export type Observation = {
  seq: number;
  fixture: FixtureKind;
  method: "GET" | "POST" | "OPTIONS";
  path: "/bootstrap" | "/login" | "/items/7" | "/write";
  status: number;
  origin_class: OriginClass;
  allow_origin_present: boolean;
  route_calls: number;
  verifier_calls: number;
  claim_committed: boolean | null;
  ready_rows: number | null;
  original_prelogin_unexpired: boolean | null;
};

const observationKeys = [
  "seq", "fixture", "method", "path", "status", "origin_class",
  "allow_origin_present", "route_calls", "verifier_calls", "claim_committed", "ready_rows", "original_prelogin_unexpired",
].sort();

function isCount(value: unknown): value is number {
  return Number.isInteger(value) && (value as number) >= 0 && (value as number) <= 10_000;
}

export async function readObservations(): Promise<Observation[]> {
  const file = process.env.SF08_OBSERVATION_FILE;
  if (!file) throw new Error("SF08 fixture observation contract is not configured");
  let handle: FileHandle | undefined;
  let bytes: Buffer;
  try {
    // Linux fixture: a FIFO must not block before fstat can reject it.
    // Inspect the opened fd, not a path checked earlier (replacement may race).
    handle = await open(file, constants.O_RDONLY | constants.O_NONBLOCK);
    if (!(await handle.stat()).isFile()) throw new Error("regular observation file required");
    // Allocate/read at most bound+1; an oversized input is never read wholesale.
    const buffer = Buffer.alloc(32 * 1024 + 1);
    let count = 0;
    while (count < buffer.length) {
      const { bytesRead } = await handle.read(buffer, count, buffer.length - count, count);
      if (bytesRead === 0) break;
      count += bytesRead;
    }
    bytes = buffer.subarray(0, count);
  } catch {
    throw new Error("SF08 fixture observation snapshot is unavailable");
  } finally {
    await handle?.close();
  }
  return parseObservations(bytes);
}

// Closed observer protocol only; presence/type does not mean the original
// token is unexpired. The replay business assertion must require true later.
export function parseObservations(bytes: Buffer): Observation[] {
  if (bytes.byteLength > 32 * 1024) throw new Error("SF08 fixture observations exceed the 32 KiB bound");

  let document: unknown;
  try {
    document = JSON.parse(bytes.toString("utf8"));
  } catch {
    throw new Error("SF08 fixture observation snapshot is malformed");
  }
  if (!document || typeof document !== "object" || Array.isArray(document)) {
    throw new Error("SF08 fixture observation snapshot has an invalid shape");
  }
  const root = document as Record<string, unknown>;
  if (Object.keys(root).sort().join("|") !== "events|schema_version"
    || root.schema_version !== 2 || !Array.isArray(root.events) || root.events.length > 64) {
    throw new Error("SF08 fixture observation snapshot is outside the bounded contract");
  }

  const events: Observation[] = [];
  let previous = 0;
  for (const raw of root.events) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
      throw new Error("SF08 fixture observation entry has an invalid shape");
    }
    const event = raw as Record<string, unknown>;
    if (Object.keys(event).sort().join("|") !== observationKeys.join("|")) {
      throw new Error("SF08 fixture observation entry has fields outside the bounded contract");
    }
    const valid = Number.isInteger(event.seq) && (event.seq as number) === previous + 1 && (event.seq as number) <= 64
      && (event.fixture === "login" || event.fixture === "cors")
      && (event.method === "GET" || event.method === "POST" || event.method === "OPTIONS")
      && (event.path === "/bootstrap" || event.path === "/login" || event.path === "/items/7" || event.path === "/write")
      && Number.isInteger(event.status) && (event.status as number) >= 100 && (event.status as number) <= 599
      && (event.origin_class === "same_origin" || event.origin_class === "allowed" || event.origin_class === "denied")
      && typeof event.allow_origin_present === "boolean"
      && isCount(event.route_calls) && isCount(event.verifier_calls)
      && (event.claim_committed === null || typeof event.claim_committed === "boolean")
      && (event.ready_rows === null || isCount(event.ready_rows))
      && (event.original_prelogin_unexpired === null || typeof event.original_prelogin_unexpired === "boolean");
    if (!valid) throw new Error("SF08 fixture observation entry failed bounded field validation");
    const loginShape = event.fixture === "login"
      && (event.path === "/bootstrap" && event.method === "GET" || event.path === "/login" && event.method === "POST")
      && event.origin_class === "same_origin" && event.allow_origin_present === false
      && event.claim_committed !== null && event.ready_rows !== null && event.original_prelogin_unexpired !== null;
    const corsShape = event.fixture === "cors"
      && (event.path === "/items/7" && event.method === "GET"
        || event.path === "/write" && (event.method === "OPTIONS" || event.method === "POST"))
      && (event.origin_class === "allowed" || event.origin_class === "denied")
      && event.allow_origin_present === (event.origin_class === "allowed")
      && event.claim_committed === null && event.ready_rows === null && event.original_prelogin_unexpired === null;
    if (!(loginShape || corsShape)) {
      throw new Error("SF08 fixture observation fields do not match the fixed route contract");
    }
    previous = event.seq as number;
    events.push(event as Observation);
  }
  return events;
}

export function lastSequence(events: Observation[]): number {
  return events.length ? events[events.length - 1].seq : 0;
}

export function latestFor(events: Observation[], fixture: FixtureKind): Observation | undefined {
  for (let index = events.length - 1; index >= 0; index--) {
    if (events[index].fixture === fixture) return events[index];
  }
  return undefined;
}

export async function waitForNew(
  afterSeq: number,
  count: number,
  predicate: (event: Observation) => boolean,
): Promise<Observation[]> {
  const deadline = Date.now() + 4_000;
  while (Date.now() < deadline) {
    const matches = (await readObservations()).filter((event) => event.seq > afterSeq && predicate(event));
    if (matches.length >= count) return matches.slice(0, count);
    await new Promise((resolve) => setTimeout(resolve, 80));
  }
  throw new Error("SF08 fixture did not record the expected bounded HTTP observation");
}
