-- The front of an organisation as a channel: a session or a call that the
-- front of the organisation the process belongs to handed over, after it
-- logged the account in. `recorded_by` is `<organisation id>:<sub>`. Widening
-- the check changes no stored row, so the append-only triggers of 0001 stand.
ALTER TABLE regelhet.grams DROP CONSTRAINT grams_recorded_via_check;
ALTER TABLE regelhet.grams ADD CONSTRAINT grams_recorded_via_check
    CHECK (recorded_via IN ('keycloak', 'testlogin', 'fsc4b', 'beheerpaneel', 'beleid', 'koppeling', 'organisatie'));
