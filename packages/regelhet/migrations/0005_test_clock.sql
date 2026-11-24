-- The test clock (src/clock.rs): in test mode the day an admin set the
-- clock to, kept beside the chronicles so every instance on this database
-- runs its steps on the same day. An instance for citizens and one for the
-- back office tell the same date. One row at most; no row means the clock
-- was never set, and an instance starts on the case's `speeldatum`. Stable
-- mode runs on the real date and never reads it.
CREATE TABLE regelhet.test_clock (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    day DATE NOT NULL,
    set_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
