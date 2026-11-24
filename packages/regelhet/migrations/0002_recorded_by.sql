-- Who asked for the step that recorded a gram: the OIDC `sub` of the
-- account, beside the gram and not inside it. NULL for what the server
-- recorded itself (the case's beginning) and for every gram stored before
-- the server had a login. Adding a column changes no stored row, so the
-- append-only triggers of 0001 stand.
ALTER TABLE regelhet.grams ADD COLUMN recorded_by TEXT;
