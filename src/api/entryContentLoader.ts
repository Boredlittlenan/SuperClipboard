type EntryContentFetcher = (id: number) => Promise<string | null>;

interface PendingRequest {
  id: number;
  resolve: (content: string | null) => void;
  reject: (reason?: unknown) => void;
}

export function createEntryContentLoader(fetcher: EntryContentFetcher, maxConcurrent = 1) {
  if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
    throw new Error('maxConcurrent must be a positive integer');
  }

  const queue: PendingRequest[] = [];
  const inFlight = new Map<number, Promise<string | null>>();
  let activeRequests = 0;

  const drain = () => {
    while (activeRequests < maxConcurrent && queue.length > 0) {
      const request = queue.shift();
      if (!request) return;
      activeRequests += 1;

      void Promise.resolve()
        .then(() => fetcher(request.id))
        .then(request.resolve, request.reject)
        .finally(() => {
          activeRequests -= 1;
          inFlight.delete(request.id);
          drain();
        });
    }
  };

  return (id: number): Promise<string | null> => {
    const existing = inFlight.get(id);
    if (existing) return existing;

    let resolve!: (content: string | null) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<string | null>((promiseResolve, promiseReject) => {
      resolve = promiseResolve;
      reject = promiseReject;
    });

    inFlight.set(id, promise);
    queue.push({ id, resolve, reject });
    drain();
    return promise;
  };
}
