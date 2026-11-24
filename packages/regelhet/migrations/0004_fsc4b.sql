-- The FSC voor Burgers channel (src/fsc4b/). Both tables hold short-lived
-- rows with their end as a Unix time in seconds; the server removes ended
-- rows as it goes. They exist whether or not the channel is configured.

-- A challenge of CreateSession, under its nonce, until the app answers it.
-- The answer deletes the row and reads it in one statement, so a nonce is
-- used once even with several instances of the server.
CREATE TABLE wetsproces.fsc4b_challenges (
    nonce TEXT PRIMARY KEY,
    -- The private scalar of the organization's ephemeral ECDH key.
    ephemeral_key BYTEA NOT NULL,
    expires_at BIGINT NOT NULL
);

CREATE INDEX fsc4b_challenges_expires_at ON wetsproces.fsc4b_challenges (expires_at);

-- A session of CreateSession, under the SHA-256 of its token: the token
-- itself is never stored.
CREATE TABLE wetsproces.fsc4b_sessions (
    token_hash BYTEA PRIMARY KEY,
    -- The session's AES key, which encrypts its traffic.
    aes_key BYTEA NOT NULL,
    bsn TEXT NOT NULL,
    scope TEXT NOT NULL,
    -- The RFC 7638 thumbprint of the app's key, the account it acts as.
    app_key_thumbprint TEXT NOT NULL,
    expires_at BIGINT NOT NULL
);

CREATE INDEX fsc4b_sessions_expires_at ON wetsproces.fsc4b_sessions (expires_at);
