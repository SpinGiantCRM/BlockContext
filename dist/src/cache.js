import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, chmodSync } from 'node:fs';
import { dirname } from 'node:path';
import { AppError } from './types.js';
// One latest value per key. No history, timers, or background refresh.
export class RequestCache {
    now;
    db;
    flights = new Map();
    failures = new Map();
    constructor(path = ':memory:', now = Date.now) {
        this.now = now;
        if (path !== ':memory:') {
            mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
        }
        this.db = new DatabaseSync(path);
        this.db.exec('PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS snapshot_checks (key TEXT PRIMARY KEY, checked_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, value TEXT NOT NULL, fetched_at INTEGER NOT NULL, expires_at INTEGER NOT NULL);');
        if (path !== ':memory:')
            chmodSync(path, 0o600);
    }
    async get(key, freshMs, maxAgeMs, load, snapshotTime) {
        const now = this.now();
        this.db.prepare('DELETE FROM cache WHERE expires_at <= ?').run(now);
        const row = this.db.prepare('SELECT value, fetched_at FROM cache WHERE key = ?').get(key);
        const existing = row ? { value: JSON.parse(row.value), fetchedAt: row.fetched_at } : undefined;
        if (existing && now - existing.fetchedAt < freshMs && now >= existing.fetchedAt)
            return { ...existing, cache: 'fresh' };
        const stale = (error) => {
            const failureTime = this.now();
            if (existing && error.transient && failureTime >= existing.fetchedAt && failureTime - existing.fetchedAt < maxAgeMs)
                return { ...existing, cache: 'stale', warning: `${error.code}: refresh failed; using an explicitly stale snapshot.` };
            throw error;
        };
        const failed = this.failures.get(key);
        if (failed && failed.until > now)
            return stale(failed.error);
        const flight = this.flights.get(key);
        if (flight)
            return flight;
        // Defer load until the flight is registered, including synchronous loader failures.
        const promise = Promise.resolve().then(load).then(value => {
            const fetchedAt = snapshotTime ? snapshotTime(value) : this.now();
            if (!Number.isFinite(fetchedAt) || fetchedAt > this.now() || this.now() - fetchedAt >= maxAgeMs)
                throw new AppError('SNAPSHOT_EXPIRED', 'The snapshot expired while being prepared.', true);
            const serialized = JSON.stringify(value);
            if (serialized.length > 8 * 1024 * 1024)
                throw new AppError('RESPONSE_TOO_LARGE', 'The normalized snapshot exceeded its storage limit.');
            this.db.prepare('INSERT INTO cache(key, value, fetched_at, expires_at) VALUES (?, ?, ?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, fetched_at=excluded.fetched_at, expires_at=excluded.expires_at').run(key, serialized, fetchedAt, fetchedAt + maxAgeMs);
            this.db.exec('DELETE FROM cache WHERE key IN (SELECT key FROM cache ORDER BY fetched_at DESC, key LIMIT -1 OFFSET 512)');
            this.failures.delete(key);
            return { value, fetchedAt, cache: 'miss' };
        }).catch(error => {
            const safe = error instanceof AppError ? error : new AppError('INTERNAL_ERROR', 'The requested snapshot could not be prepared.');
            this.failures.set(key, { until: this.now() + Math.max(15_000, Math.min(300_000, (safe.retryAfterSeconds ?? 0) * 1000)), error: safe });
            if (this.failures.size > 512)
                this.failures.delete(this.failures.keys().next().value);
            return stale(safe);
        }).finally(() => this.flights.delete(key));
        this.flights.set(key, promise);
        return promise;
    }
    async snapshot(key, freshMs, maxAgeMs, load, time, mode = 'snapshot') {
        if (mode === 'cached') {
            const existing = this.peek(key, maxAgeMs);
            if (existing)
                return { ...existing, cache: this.now() - existing.fetchedAt < freshMs ? 'fresh' : 'stale', warning: this.now() - existing.fetchedAt >= freshMs ? 'Cached player snapshot; no API refresh was attempted.' : undefined };
            if (this.db.prepare('SELECT key FROM snapshot_checks WHERE key=?').get(key))
                throw new AppError('SNAPSHOT_REFRESH_REQUIRED', 'The cached player snapshot has expired. Request snapshot mode for an explicit on-demand check; use collector live reads for polling.');
        }
        const result = await this.get(key, freshMs, maxAgeMs, load, time);
        this.db.prepare('INSERT INTO snapshot_checks(key,checked_at) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET checked_at=excluded.checked_at').run(key, this.now());
        this.db.exec('DELETE FROM snapshot_checks WHERE key IN (SELECT key FROM snapshot_checks ORDER BY checked_at DESC LIMIT -1 OFFSET 2048)');
        return result;
    }
    peek(key, maxAgeMs) {
        const row = this.db.prepare('SELECT value,fetched_at FROM cache WHERE key = ?').get(key);
        if (!row || this.now() < row.fetched_at || this.now() - row.fetched_at >= maxAgeMs)
            return null;
        return { value: JSON.parse(row.value), fetchedAt: row.fetched_at, cache: 'stale' };
    }
    close() { this.db.close(); }
}
