-- The channel the account of `recorded_by` came in by: the login through
-- Keycloak, the test login, or FSC voor Burgers. NULL beside a NULL
-- `recorded_by`, and for every gram stored before the channel was kept.
-- Adding a column changes no stored row, so the append-only triggers of 0001
-- stand.
ALTER TABLE regelhet.grams ADD COLUMN recorded_via TEXT
    CHECK (recorded_via IN ('keycloak', 'testlogin', 'fsc4b'));
