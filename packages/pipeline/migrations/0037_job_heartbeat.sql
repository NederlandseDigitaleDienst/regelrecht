-- A worker that holds a job now says so every 30 seconds. The orphan reaper
-- used to reset every job that had been processing longer than the orphan
-- timeout, measured from `started_at`. A run that was still working past that
-- point was put back to pending, and its result was then refused because the
-- job was no longer processing: the whole run was lost and started again.
-- The reaper now looks at the last heartbeat instead, so it only takes back
-- jobs whose worker has stopped responding.
ALTER TABLE jobs ADD COLUMN heartbeat_at TIMESTAMPTZ;
