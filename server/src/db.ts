import { Database } from "bun:sqlite";

export type OrderStatus = "pending" | "paid" | "failed" | "expired" | "refunded";

export type Order = {
  ref: string;
  email: string;
  checkout_id: string | null;
  amount_cents: number;
  mode: "test" | "live";
  status: OrderStatus;
  token: string | null;
  created: number;
  paid: number | null;
};

// ponytail: schema created per process; add migrations at the first schema change
export function openDb(path: string): Database {
  const db = new Database(path, { create: true, strict: true });
  db.run("PRAGMA journal_mode = WAL");
  db.run("PRAGMA busy_timeout = 5000");
  db.run(`CREATE TABLE IF NOT EXISTS orders (
    ref TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    checkout_id TEXT,
    amount_cents INTEGER NOT NULL,
    mode TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending','paid','failed','expired','refunded')),
    token TEXT,
    created INTEGER NOT NULL,
    paid INTEGER
  )`);
  db.run("CREATE INDEX IF NOT EXISTS orders_email ON orders (email)");
  // The plain email is kept only so /recover can find the trial token.
  db.run(`CREATE TABLE IF NOT EXISTS trials (
    email_hash TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    token TEXT NOT NULL,
    created INTEGER NOT NULL
  )`);
  return db;
}
