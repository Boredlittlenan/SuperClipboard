import { describe, expect, it, vi } from 'vitest';
import { createEntryContentLoader } from './entryContentLoader';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
}

describe('createEntryContentLoader', () => {
  it('serializes image loads and deduplicates requests for the same entry', async () => {
    const firstResult = deferred<string | null>();
    const secondResult = deferred<string | null>();
    const fetcher = vi
      .fn<(id: number) => Promise<string | null>>()
      .mockReturnValueOnce(firstResult.promise)
      .mockReturnValueOnce(secondResult.promise);
    const load = createEntryContentLoader(fetcher);

    const first = load(1);
    const duplicate = load(1);
    const second = load(2);

    expect(duplicate).toBe(first);
    await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));

    firstResult.resolve('first');
    await expect(first).resolves.toBe('first');
    await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));

    secondResult.resolve('second');
    await expect(second).resolves.toBe('second');
  });

  it('continues draining the queue after a failed request', async () => {
    const fetcher = vi
      .fn<(id: number) => Promise<string | null>>()
      .mockRejectedValueOnce(new Error('network error'))
      .mockResolvedValueOnce('recovered');
    const load = createEntryContentLoader(fetcher);

    await expect(load(1)).rejects.toThrow('network error');
    await expect(load(2)).resolves.toBe('recovered');
    expect(fetcher).toHaveBeenCalledTimes(2);
  });
});
