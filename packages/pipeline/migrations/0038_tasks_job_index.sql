-- Index every task by its job, whatever its status. The traject runs overview
-- counts a run's review tasks per status (open, approved, rejected,
-- dismissed); the partial index on open tasks only covered the first, so each
-- run cost a scan of the whole table. This index serves the open-task lookups
-- too, so the partial one goes.
CREATE INDEX idx_tasks_job ON tasks (job_id);
DROP INDEX IF EXISTS idx_tasks_job_open;
