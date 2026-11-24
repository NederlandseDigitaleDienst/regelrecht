-- The executor's own policy as a channel: what the server records by an
-- article of that policy when an application comes in, such as the request
-- to the GGD for an examination and the GGD's receipt of it. No account asked
-- for it; `recorded_by` names the article (`<regelwerk>#<article>`). Widening
-- the check changes no stored row, so the append-only triggers of 0001 stand.
ALTER TABLE wetsproces.grams DROP CONSTRAINT grams_recorded_via_check;
ALTER TABLE wetsproces.grams ADD CONSTRAINT grams_recorded_via_check
    CHECK (recorded_via IN ('keycloak', 'testlogin', 'fsc4b', 'beheerpaneel', 'beleid'));
