-- The admin panel of the back office as a channel: an admin records there
-- what an external party of the case did, such as the GGD's advice or the
-- court's judgment, until a real link with that party replaces it. The record
-- names the party; `recorded_by` names the admin's account. Widening the
-- check changes no stored row, so the append-only triggers of 0001 stand.
ALTER TABLE wetsproces.grams DROP CONSTRAINT grams_recorded_via_check;
ALTER TABLE wetsproces.grams ADD CONSTRAINT grams_recorded_via_check
    CHECK (recorded_via IN ('keycloak', 'testlogin', 'fsc4b', 'beheerpaneel'));
