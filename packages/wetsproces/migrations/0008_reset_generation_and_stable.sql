-- Two guards around emptying a test setup (`ChronicleStore::reset`, 0007).
--
-- The generation: how often the store was reset. A step reads it with its
-- chronicles and its commit names it, so a step read before a reset is
-- refused even when the heads stand where they stood before it. A reset
-- raises it in the transaction that empties the chronicles; nothing else
-- writes it.
CREATE TABLE wetsproces.generation (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    generation BIGINT NOT NULL CHECK (generation >= 0)
);
INSERT INTO wetsproces.generation (generation) VALUES (0);

-- A server in stable mode marks the database it uses here when it starts.
-- From then on the triggers of the chronicles refuse the TRUNCATE that 0007
-- lets through, so a reset can never empty a database that stable mode ran
-- on, whatever the server or its configuration does. Like 0007 this guards
-- against a mistake in the server, not against someone who can run SQL here.
CREATE TABLE wetsproces.stable (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    since TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION wetsproces.refuse_changing_grams() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE'
        AND current_setting('wetsproces.leegmaken', true) = 'testomgeving'
        AND NOT EXISTS (SELECT 1 FROM wetsproces.stable) THEN
        RETURN NULL;
    END IF;
    RAISE EXCEPTION 'wetsproces.grams is append-only: % is not allowed', TG_OP
        USING ERRCODE = 'restrict_violation';
END;
$$;
