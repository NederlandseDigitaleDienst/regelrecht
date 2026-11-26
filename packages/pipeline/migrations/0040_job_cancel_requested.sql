-- When a member asked to stop a processing run. A pending run is deleted
-- outright; a processing one is held by a worker, so the request is recorded
-- here and the worker's heartbeat picks it up, kills the agent and marks the
-- job `cancelled`. `fail_job`, `fail_job_terminal` and the orphan reaper read
-- it too, so a run that fails or loses its worker after the request ends as
-- cancelled instead of being retried.
--
-- The active-enrich unique index (0029) covers pending and processing only, so
-- a cancelled job no longer blocks a new request for the same law.
ALTER TABLE jobs ADD COLUMN cancel_requested_at TIMESTAMPTZ;
