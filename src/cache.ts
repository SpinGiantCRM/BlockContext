import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, chmodSync } from 'node:fs';
import { dirname } from 'node:path';
import { AppError } from './types.js';

export interface Cached<T> { value: T; fetchedAt: number; cache: 'fresh' | 'miss' | 'stale'; warning?: string }
// One latest value per key. No history, timers, or background refresh.
export class RequestCache {
  private readonly db: DatabaseSync;
  private readonly flights = new Map<string, Promise<Cached<unknown>>>();
  private readonly failures = new Map<string, { until: number; error: AppError }>();
  constructor(path = ':memory:', private readonly now = Date.now) {
    if (path !== ':memory:') { mkdirSync(dirname(path), { recursive: true, mode: 0o700 }); }
    this.db = new DatabaseSync(path);
    this.db.exec('PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, value TEXT NOT NULL, fetched_at INTEGER NOT NULL, expires_at INTEGER NOT NULL);');
    if (path !== ':memory:') chmodSync(path, 0o600);
  }
  async get<T>(key: string, freshMs: number, maxAgeMs: number, load: () => Promise<T>, snapshotTime?: (value: T) => number): Promise<Cached<T>> {
    const now = this.now();
    this.db.prepare('DELETE FROM cache WHERE expires_at <= ?').run(now);
    const row = this.db.prepare('SELECT value, fetched_at FROM cache WHERE key = ?').get(key) as { value: string; fetched_at: number } | undefined;
    const existing = row ? { value: JSON.parse(row.value) as T, fetchedAt: row.fetched_at } : undefined;
    if (existing && now - existing.fetchedAt < freshMs && now >= existing.fetchedAt) return { ...existing, cache: 'fresh' };
    const stale = (error: AppError): Cached<T> => {
      const failureTime = this.now();
      if (existing && error.transient && failureTime >= existing.fetchedAt && failureTime - existing.fetchedAt < maxAgeMs)
        return { ...existing, cache: 'stale', warning: `${error.code}: refresh failed; using an explicitly stale snapshot.` };
      throw error;
    };
    const failed = this.failures.get(key);
    if (failed && failed.until > now) return stale(failed.error);
    const flight = this.flights.get(key);
    if (flight) return flight as Promise<Cached<T>>;
    // Defer load until the flight is registered, including synchronous loader failures.
    const promise = Promise.resolve().then(load).then(value => {
      const fetchedAt = snapshotTime ? snapshotTime(value) : this.now();
      if (!Number.isFinite(fetchedAt) || fetchedAt > this.now() || this.now() - fetchedAt >= maxAgeMs) throw new AppError('SNAPSHOT_EXPIRED', 'The snapshot expired while being prepared.', true);
      const serialized = JSON.stringify(value);
      if (serialized.length > 8 * 1024 * 1024) throw new AppError('RESPONSE_TOO_LARGE', 'The normalized snapshot exceeded its storage limit.');
      this.db.prepare('INSERT INTO cache(key, value, fetched_at, expires_at) VALUES (?, ?, ?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, fetched_at=excluded.fetched_at, expires_at=excluded.expires_at').run(key, serialized, fetchedAt, fetchedAt + maxAgeMs);
      this.db.exec('DELETE FROM cache WHERE key IN (SELECT key FROM cache ORDER BY fetched_at DESC, key LIMIT -1 OFFSET 512)');
      this.failures.delete(key);
      return { value, fetchedAt, cache: 'miss' as const };
    }).catch(error => {
      const safe = error instanceof AppError ? error : new AppError('INTERNAL_ERROR', 'The requested snapshot could not be prepared.');
      this.failures.set(key, { until: this.now() + Math.max(15_000, Math.min(300_000, (safe.retryAfterSeconds ?? 0) * 1000)), error: safe });
      if (this.failures.size > 512) this.failures.delete(this.failures.keys().next().value!);
      return stale(safe);
    }).finally(() => this.flights.delete(key));
    this.flights.set(key, promise);
    return promise;
  }
  close() { this.db.close(); }
}
