-- The link with the server of another cell of the case (src/link.rs).
--
-- A record such a server made and sent arrives through the channel
-- `koppeling`; `recorded_by` names the cell it came from. Widening the check
-- changes no stored row, so the append-only triggers of 0001 stand.
ALTER TABLE wetsproces.grams DROP CONSTRAINT grams_recorded_via_check;
ALTER TABLE wetsproces.grams ADD CONSTRAINT grams_recorded_via_check
    CHECK (recorded_via IN ('keycloak', 'testlogin', 'fsc4b', 'beheerpaneel', 'beleid', 'koppeling'));

-- Which records of this server's own cell reached the server of another
-- cell. The chronicle is the outbox: a record whose article has that cell
-- record a receipt is to be sent until it stands here. A reset of test mode
-- empties it with the chronicles.
CREATE TABLE wetsproces.link_deliveries (
    gram_id TEXT NOT NULL,
    to_cell TEXT NOT NULL,
    delivered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (gram_id, to_cell)
);
