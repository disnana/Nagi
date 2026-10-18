import { lstat, realpath } from "node:fs/promises";
import { createConnection, type Socket } from "node:net";
import { isAbsolute, join, resolve } from "node:path";

const RESPONSE_LIMIT = 128;
const RESET_DEADLINE_MS = 30_000;

function unavailable(): Error {
  return new Error("SF08 fixture reset control is unavailable or outside its bounded contract");
}

async function ownedSocketPath(): Promise<string> {
  if (process.platform !== "linux" || typeof process.geteuid !== "function") throw unavailable();
  const work = process.env.SF08_WORK_DIR;
  const control = process.env.SF08_CONTROL_SOCKET;
  if (!work || !control || !isAbsolute(work) || !isAbsolute(control)) throw unavailable();

  try {
    const workStat = await lstat(work);
    const canonicalWork = await realpath(work);
    if (!workStat.isDirectory() || workStat.isSymbolicLink()
      || canonicalWork !== resolve(work) || workStat.uid !== process.geteuid()
      || (workStat.mode & 0o777) !== 0o700) throw unavailable();

    const expectedSocket = join(canonicalWork, "control.sock");
    if (resolve(control) !== expectedSocket) throw unavailable();
    const socketStat = await lstat(expectedSocket);
    if (!socketStat.isSocket() || socketStat.isSymbolicLink()
      || socketStat.uid !== process.geteuid() || (socketStat.mode & 0o777) !== 0o600) {
      throw unavailable();
    }
    return expectedSocket;
  } catch {
    throw unavailable();
  }
}

function parseReadyResponse(bytes: Buffer): void {
  if (bytes.byteLength > RESPONSE_LIMIT || bytes.length < 2 || bytes[bytes.length - 1] !== 0x0a) {
    throw unavailable();
  }
  if (bytes.subarray(0, bytes.length - 1).includes(0x0a) || bytes.includes(0x0d)) {
    throw unavailable();
  }

  const text = bytes.subarray(0, bytes.length - 1).toString("utf8");
  if (!/^[\t ]*\{[\t ]*"ready"[\t ]*:[\t ]*true[\t ]*\}$/.test(text)) throw unavailable();
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw unavailable();
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) throw unavailable();
  const record = value as Record<string, unknown>;
  if (Object.keys(record).length !== 1 || Object.keys(record)[0] !== "ready" || record.ready !== true) {
    throw unavailable();
  }
}

export async function resetFixture(signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) throw new Error("SF08 fixture reset was cancelled");
  const socketPath = await ownedSocketPath();
  if (signal?.aborted) throw new Error("SF08 fixture reset was cancelled");
  await new Promise<void>((resolvePromise, rejectPromise) => {
    const socket: Socket = createConnection({ path: socketPath });
    const chunks: Buffer[] = [];
    let received = 0;
    let ended = false;
    let closed = false;
    let settled = false;
    let failure: Error | undefined;

    const timer = setTimeout(() => fail(new Error("SF08 fixture reset exceeded its bounded deadline")), RESET_DEADLINE_MS);
    const cleanup = () => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
      socket.removeListener("connect", onConnect);
      socket.removeListener("data", onData);
      socket.removeListener("end", onEnd);
      socket.removeListener("error", onError);
      socket.removeListener("close", onClose);
    };
    const finishFailure = () => {
      if (settled || !failure || !closed) return;
      settled = true;
      cleanup();
      rejectPromise(failure);
    };
    const fail = (error: Error) => {
      if (settled || failure) return;
      failure = error;
      if (!socket.destroyed) socket.destroy();
      finishFailure();
    };
    const onAbort = () => fail(new Error("SF08 fixture reset was cancelled"));
    const onConnect = () => {
      socket.setNoDelay(true);
      socket.write('{"op":"reset"}\n', (error) => {
        if (error) fail(unavailable());
      });
    };
    const onData = (chunk: Buffer) => {
      received += chunk.byteLength;
      if (received > RESPONSE_LIMIT) {
        fail(unavailable());
        return;
      }
      chunks.push(Buffer.from(chunk));
    };
    const onEnd = () => {
      ended = true;
      try {
        parseReadyResponse(Buffer.concat(chunks, received));
        if (!settled) {
          // The producer closes after its single response; explicitly finish the local fd too.
          if (closed) {
            settled = true;
            cleanup();
            resolvePromise();
          } else {
            socket.once("close", () => {
              if (!settled) {
                settled = true;
                cleanup();
                resolvePromise();
              }
            });
            socket.destroy();
          }
        }
      } catch {
        fail(unavailable());
      }
    };
    const onError = () => fail(unavailable());
    const onClose = () => {
      closed = true;
      if (failure) finishFailure();
      else if (!ended) fail(unavailable());
    };

    socket.on("connect", onConnect);
    socket.on("data", onData);
    socket.on("end", onEnd);
    socket.on("error", onError);
    socket.on("close", onClose);
    signal?.addEventListener("abort", onAbort, { once: true });
    if (signal?.aborted) onAbort();
  });
}
