-- The chronicles of the cells, in the schema `wetsproces`. The store creates
-- the schema before it migrates and connects with it first in its
-- search_path, so sqlx's migration table lives here as well. Nothing in this
-- schema refers to another one.

-- Per cell its head: the number of records in its chronicle. A commit locks
-- the rows of its cells, so commits on one cell take turns.
CREATE TABLE wetsproces.heads (
    cell TEXT PRIMARY KEY,
    head BIGINT NOT NULL CHECK (head >= 0)
);

-- One row per record, append-only. `seq` counts from 1 per cell;
-- `format_version` says how `gram` is to be read.
CREATE TABLE wetsproces.grams (
    cell TEXT NOT NULL REFERENCES wetsproces.heads (cell),
    seq BIGINT NOT NULL CHECK (seq >= 1),
    gram_id TEXT NOT NULL UNIQUE,
    format_version INTEGER NOT NULL,
    gram JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (cell, seq)
);

-- What a request under an idempotency key answered.
CREATE TABLE wetsproces.receipts (
    key TEXT PRIMARY KEY,
    request JSONB NOT NULL,
    response JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A chronicle only grows: a stored record is never changed or removed.
CREATE FUNCTION wetsproces.refuse_changing_grams() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'wetsproces.grams is append-only: % is not allowed', TG_OP
        USING ERRCODE = 'restrict_violation';
END;
$$;

CREATE TRIGGER grams_append_only
    BEFORE UPDATE OR DELETE ON wetsproces.grams
    FOR EACH ROW EXECUTE FUNCTION wetsproces.refuse_changing_grams();

CREATE TRIGGER grams_no_truncate
    BEFORE TRUNCATE ON wetsproces.grams
    FOR EACH STATEMENT EXECUTE FUNCTION wetsproces.refuse_changing_grams();
