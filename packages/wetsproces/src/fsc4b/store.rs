//! Where the channel keeps its challenges and sessions: in memory beside
//! the chronicles in memory, and in the schema `wetsproces` beside the
//! chronicles in Postgres (migration 0004), so every instance of the server
//! sees the same ones.
//!
//! A challenge is taken once: [`Fsc4bStore::take_challenge`] removes it as
//! it reads it, in one step, so no two answers to one nonce both succeed,
//! however many instances run. A session is kept under the SHA-256 of its
//! token, never under the token itself. Both carry their end as a Unix time;
//! a store gives nothing that has ended, and removes ended rows as it goes.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

use crate::fsc4b::jose::AesKey;

/// A challenge waiting for its answer.
#[derive(Debug)]
pub struct StoredChallenge {
    /// The private scalar of the ephemeral ECDH key.
    pub ephemeral_key: Vec<u8>,
    pub expires_at: i64,
}

/// A session of CreateSession.
#[derive(Debug, Clone)]
pub struct StoredSession {
    pub aes_key: AesKey,
    pub bsn: String,
    pub scope: String,
    /// The RFC 7638 thumbprint of the app's key.
    pub app_key_thumbprint: String,
    pub expires_at: i64,
}

/// The SHA-256 of a session token: what a session is kept and found under.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TokenHash([u8; 32]);

impl TokenHash {
    pub fn of(token: &str) -> Self {
        Self(Sha256::digest(token.as_bytes()).into())
    }
}

/// How many challenges may wait at once, on all instances together. Asking
/// a challenge needs no login, so without a bound anyone could fill the
/// store faster than the minute a challenge lasts empties it.
pub const MAX_LIVE_CHALLENGES: i64 = 10_000;

#[derive(Debug, thiserror::Error)]
pub enum Fsc4bStoreError {
    /// [`MAX_LIVE_CHALLENGES`] challenges wait already.
    #[error("{MAX_LIVE_CHALLENGES} challenges wait already")]
    Full,
    #[error("the FSC voor Burgers store: {0}")]
    Backend(String),
}

/// The challenges and sessions of the channel.
#[async_trait]
pub trait Fsc4bStore: Send + Sync + std::fmt::Debug {
    /// Keep `challenge` under `nonce`.
    ///
    /// # Errors
    ///
    /// When the store fails, or the nonce is already kept.
    async fn put_challenge(
        &self,
        nonce: &str,
        challenge: StoredChallenge,
        now: i64,
    ) -> Result<(), Fsc4bStoreError>;

    /// The challenge under `nonce`, removed as it is read; `None` when
    /// there is none, it was taken before, or it ended before `now`.
    ///
    /// # Errors
    ///
    /// When the store fails.
    async fn take_challenge(
        &self,
        nonce: &str,
        now: i64,
    ) -> Result<Option<StoredChallenge>, Fsc4bStoreError>;

    /// Keep `session` under `token`.
    ///
    /// # Errors
    ///
    /// When the store fails.
    async fn put_session(
        &self,
        token: &TokenHash,
        session: StoredSession,
        now: i64,
    ) -> Result<(), Fsc4bStoreError>;

    /// The session under `token`, when it has not ended at `now`.
    ///
    /// # Errors
    ///
    /// When the store fails.
    async fn session(
        &self,
        token: &TokenHash,
        now: i64,
    ) -> Result<Option<StoredSession>, Fsc4bStoreError>;
}

#[derive(Debug, Default)]
struct Memory {
    challenges: BTreeMap<String, StoredChallenge>,
    sessions: BTreeMap<TokenHash, StoredSession>,
}

/// The challenges and sessions in memory, gone at every stop.
#[derive(Debug, Default)]
pub struct InMemoryFsc4bStore {
    state: Mutex<Memory>,
}

impl InMemoryFsc4bStore {
    fn state(&self) -> Result<std::sync::MutexGuard<'_, Memory>, Fsc4bStoreError> {
        // As the chronicle store in memory: a panic under the lock leaves
        // nothing to trust, so say so rather than go on.
        self.state
            .lock()
            .map_err(|_| Fsc4bStoreError::Backend("the store in memory is poisoned".into()))
    }
}

#[async_trait]
impl Fsc4bStore for InMemoryFsc4bStore {
    async fn put_challenge(
        &self,
        nonce: &str,
        challenge: StoredChallenge,
        now: i64,
    ) -> Result<(), Fsc4bStoreError> {
        let mut state = self.state()?;
        state.challenges.retain(|_, c| c.expires_at > now);
        if i64::try_from(state.challenges.len()).unwrap_or(i64::MAX) >= MAX_LIVE_CHALLENGES {
            return Err(Fsc4bStoreError::Full);
        }
        if state.challenges.contains_key(nonce) {
            return Err(Fsc4bStoreError::Backend("the nonce is already kept".into()));
        }
        state.challenges.insert(nonce.to_string(), challenge);
        drop(state);
        Ok(())
    }

    async fn take_challenge(
        &self,
        nonce: &str,
        now: i64,
    ) -> Result<Option<StoredChallenge>, Fsc4bStoreError> {
        Ok(self
            .state()?
            .challenges
            .remove(nonce)
            .filter(|c| c.expires_at > now))
    }

    async fn put_session(
        &self,
        token: &TokenHash,
        session: StoredSession,
        now: i64,
    ) -> Result<(), Fsc4bStoreError> {
        let mut state = self.state()?;
        state.sessions.retain(|_, s| s.expires_at > now);
        state.sessions.insert(token.clone(), session);
        drop(state);
        Ok(())
    }

    async fn session(
        &self,
        token: &TokenHash,
        now: i64,
    ) -> Result<Option<StoredSession>, Fsc4bStoreError> {
        Ok(self
            .state()?
            .sessions
            .get(token)
            .filter(|s| s.expires_at > now)
            .cloned())
    }
}

/// The challenges and sessions in Postgres, in the tables of migration
/// 0004.
#[derive(Clone)]
pub struct PostgresFsc4bStore {
    pool: PgPool,
}

impl std::fmt::Debug for PostgresFsc4bStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PostgresFsc4bStore")
    }
}

impl PostgresFsc4bStore {
    /// The store on `pool`, whose schema the chronicle store migrated.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn failed(context: &'static str) -> impl FnOnce(sqlx::Error) -> Fsc4bStoreError {
    move |e| Fsc4bStoreError::Backend(format!("{context}: {e}"))
}

#[async_trait]
impl Fsc4bStore for PostgresFsc4bStore {
    async fn put_challenge(
        &self,
        nonce: &str,
        challenge: StoredChallenge,
        now: i64,
    ) -> Result<(), Fsc4bStoreError> {
        sqlx::query("DELETE FROM wetsproces.fsc4b_challenges WHERE expires_at <= $1")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(failed("removing ended challenges"))?;
        // The bound and the insert in one statement. Two instances inserting
        // at once may each see one row short, which the bound allows for.
        let inserted = sqlx::query(
            "INSERT INTO wetsproces.fsc4b_challenges (nonce, ephemeral_key, expires_at) \
             SELECT $1, $2, $3 \
             WHERE (SELECT count(*) FROM wetsproces.fsc4b_challenges) < $4",
        )
        .bind(nonce)
        .bind(&challenge.ephemeral_key)
        .bind(challenge.expires_at)
        .bind(MAX_LIVE_CHALLENGES)
        .execute(&self.pool)
        .await
        .map_err(failed("keeping a challenge"))?
        .rows_affected();
        if inserted == 0 {
            return Err(Fsc4bStoreError::Full);
        }
        Ok(())
    }

    async fn take_challenge(
        &self,
        nonce: &str,
        now: i64,
    ) -> Result<Option<StoredChallenge>, Fsc4bStoreError> {
        // One statement: of two instances taking the same nonce, one gets
        // the row and the other nothing.
        let row = sqlx::query(
            "DELETE FROM wetsproces.fsc4b_challenges WHERE nonce = $1 \
             RETURNING ephemeral_key, expires_at",
        )
        .bind(nonce)
        .fetch_optional(&self.pool)
        .await
        .map_err(failed("taking a challenge"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let challenge = StoredChallenge {
            ephemeral_key: row
                .try_get("ephemeral_key")
                .map_err(failed("reading a challenge"))?,
            expires_at: row
                .try_get("expires_at")
                .map_err(failed("reading a challenge"))?,
        };
        Ok(Some(challenge).filter(|c| c.expires_at > now))
    }

    async fn put_session(
        &self,
        token: &TokenHash,
        session: StoredSession,
        now: i64,
    ) -> Result<(), Fsc4bStoreError> {
        sqlx::query("DELETE FROM wetsproces.fsc4b_sessions WHERE expires_at <= $1")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(failed("removing ended sessions"))?;
        sqlx::query(
            "INSERT INTO wetsproces.fsc4b_sessions \
             (token_hash, aes_key, bsn, scope, app_key_thumbprint, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&token.0[..])
        .bind(session.aes_key.as_bytes())
        .bind(&session.bsn)
        .bind(&session.scope)
        .bind(&session.app_key_thumbprint)
        .bind(session.expires_at)
        .execute(&self.pool)
        .await
        .map_err(failed("keeping a session"))?;
        Ok(())
    }

    async fn session(
        &self,
        token: &TokenHash,
        now: i64,
    ) -> Result<Option<StoredSession>, Fsc4bStoreError> {
        let row = sqlx::query(
            "SELECT aes_key, bsn, scope, app_key_thumbprint, expires_at \
             FROM wetsproces.fsc4b_sessions WHERE token_hash = $1 AND expires_at > $2",
        )
        .bind(&token.0[..])
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(failed("reading a session"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let read = failed("reading a session");
        let key: Vec<u8> = row.try_get("aes_key").map_err(read)?;
        let aes_key = AesKey::from_slice(&key).map_err(|_| {
            Fsc4bStoreError::Backend("a stored session key has the wrong length".into())
        })?;
        let column = |name: &'static str| -> Result<String, Fsc4bStoreError> {
            row.try_get(name).map_err(failed("reading a session"))
        };
        Ok(Some(StoredSession {
            aes_key,
            bsn: column("bsn")?,
            scope: column("scope")?,
            app_key_thumbprint: column("app_key_thumbprint")?,
            expires_at: row
                .try_get("expires_at")
                .map_err(failed("reading a session"))?,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use regelrecht_shared::postgres::test_utils::TestDatabase;

    use super::*;
    use crate::store::postgres::PostgresStore;

    fn challenge(expires_at: i64) -> StoredChallenge {
        StoredChallenge {
            ephemeral_key: vec![1; 32],
            expires_at,
        }
    }

    fn session(expires_at: i64) -> StoredSession {
        StoredSession {
            aes_key: AesKey::from_slice(&[2; 32]).unwrap(),
            bsn: "999990019".into(),
            scope: "s".into(),
            app_key_thumbprint: "t".into(),
            expires_at,
        }
    }

    async fn a_challenge_is_taken_once_and_not_after_it_ended(store: &dyn Fsc4bStore) {
        store.put_challenge("n1", challenge(100), 0).await.unwrap();
        assert!(store.take_challenge("n1", 50).await.unwrap().is_some());
        assert!(store.take_challenge("n1", 50).await.unwrap().is_none());
        store.put_challenge("n2", challenge(100), 0).await.unwrap();
        assert!(store.take_challenge("n2", 100).await.unwrap().is_none());
        // Taken after its end, it is gone all the same.
        assert!(store.take_challenge("n2", 0).await.unwrap().is_none());
        assert!(store.take_challenge("nooit", 0).await.unwrap().is_none());
    }

    async fn a_session_is_found_by_its_token_until_it_ends(store: &dyn Fsc4bStore) {
        let token = TokenHash::of("token");
        store.put_session(&token, session(100), 0).await.unwrap();
        let found = store.session(&token, 99).await.unwrap().unwrap();
        assert_eq!(found.bsn, "999990019");
        assert_eq!(found.aes_key.as_bytes(), &[2; 32]);
        assert!(store.session(&token, 100).await.unwrap().is_none());
        assert!(
            store
                .session(&TokenHash::of("other"), 0)
                .await
                .unwrap()
                .is_none()
        );
    }

    async fn the_number_of_waiting_challenges_is_bounded(store: &dyn Fsc4bStore) {
        for i in 0..MAX_LIVE_CHALLENGES {
            store
                .put_challenge(&format!("n{i}"), challenge(100), 0)
                .await
                .unwrap();
        }
        let e = store.put_challenge("one-more", challenge(100), 0).await;
        assert!(matches!(e, Err(Fsc4bStoreError::Full)), "{e:?}");
        // Once they ended, there is room again.
        store
            .put_challenge("later", challenge(200), 100)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn in_memory() {
        a_challenge_is_taken_once_and_not_after_it_ended(&InMemoryFsc4bStore::default()).await;
        a_session_is_found_by_its_token_until_it_ends(&InMemoryFsc4bStore::default()).await;
        the_number_of_waiting_challenges_is_bounded(&InMemoryFsc4bStore::default()).await;
    }

    #[tokio::test]
    async fn in_postgres() {
        let database = TestDatabase::create().await;
        let chronicles = PostgresStore::connect(database.url()).await.unwrap();
        let store = PostgresFsc4bStore::new(chronicles.pool().clone());
        a_challenge_is_taken_once_and_not_after_it_ended(&store).await;
        a_session_is_found_by_its_token_until_it_ends(&store).await;
    }

    #[tokio::test]
    async fn two_instances_taking_one_nonce_get_it_once() {
        let database = TestDatabase::create().await;
        let one = PostgresStore::connect(database.url()).await.unwrap();
        let two = PostgresStore::connect(database.url()).await.unwrap();
        let one = PostgresFsc4bStore::new(one.pool().clone());
        let two = PostgresFsc4bStore::new(two.pool().clone());
        one.put_challenge("n", challenge(100), 0).await.unwrap();
        let (a, b) = tokio::join!(one.take_challenge("n", 1), two.take_challenge("n", 1));
        let taken = [a.unwrap(), b.unwrap()]
            .iter()
            .filter(|c| c.is_some())
            .count();
        assert_eq!(taken, 1);
    }
}
