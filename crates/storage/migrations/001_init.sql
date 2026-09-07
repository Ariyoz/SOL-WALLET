-- Migration 001: initial schema
--
-- Security: private keys and mnemonics are NEVER stored here.
-- Only public addresses and transaction metadata.

CREATE TABLE IF NOT EXISTS wallets (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    address    TEXT    NOT NULL UNIQUE,
    label      TEXT,
    created_at TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS transactions (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    signature            TEXT    NOT NULL UNIQUE,
    wallet_address       TEXT    NOT NULL,
    counterparty_address TEXT    NOT NULL,
    direction            TEXT    NOT NULL CHECK(direction IN ('sent', 'received')),
    amount_lamports      TEXT    NOT NULL,
    fee_lamports         TEXT    NOT NULL,
    status               TEXT    NOT NULL,
    block_time           TEXT,
    slot                 INTEGER,
    created_at           TEXT    NOT NULL
);

-- Composite index for paginated history: WHERE wallet_address = ? ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_tx_wallet_time ON transactions(wallet_address, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tx_signature   ON transactions(signature);
