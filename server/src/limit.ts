// ponytail: in-memory, single instance; move to SQLite if the server runs more than one instance
export function createLimiter(now: () => number) {
  const windows = new Map<string, { start: number; count: number }>();

  /** True while `key` has made at most `max` calls in the current `windowMs` window. */
  return (key: string, max: number, windowMs: number): boolean => {
    const t = now();
    if (windows.size > 10_000) {
      for (const [k, w] of windows) if (t - w.start >= windowMs) windows.delete(k);
    }
    const w = windows.get(key);
    if (!w || t - w.start >= windowMs) {
      windows.set(key, { start: t, count: 1 });
      return true;
    }
    w.count += 1;
    return w.count <= max;
  };
}
