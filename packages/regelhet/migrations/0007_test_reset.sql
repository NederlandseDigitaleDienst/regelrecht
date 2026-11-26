-- Emptying a test setup (`ChronicleStore::reset`, only in test mode). The
-- chronicles stay append-only: the function behind the triggers of 0001
-- refuses every UPDATE, DELETE and TRUNCATE as before, except a TRUNCATE in a
-- transaction that set `wetsproces.leegmaken` to `testomgeving` itself
-- (`set_config(..., true)`, which lasts until the transaction ends). Only the
-- reset of the server sets it, and the server offers the reset only in test
-- mode. This guards against a mistake in the server, not against someone who
-- can run SQL on this database: such a user can set the setting too.
CREATE OR REPLACE FUNCTION wetsproces.refuse_changing_grams() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE'
        AND current_setting('wetsproces.leegmaken', true) = 'testomgeving' THEN
        RETURN NULL;
    END IF;
    RAISE EXCEPTION 'wetsproces.grams is append-only: % is not allowed', TG_OP
        USING ERRCODE = 'restrict_violation';
END;
$$;
