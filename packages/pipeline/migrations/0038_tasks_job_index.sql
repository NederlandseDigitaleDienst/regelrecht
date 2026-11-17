-- Indexes for the traject runs overview (pipeline::traject_runs).

-- Every task by its job, whatever its status. The overview counts a run's
-- review tasks per status (open, approved, rejected, dismissed); the partial
-- index on open tasks only covered the first, so each run cost a scan of the
-- whole table. This index serves the open-task lookups too, so the partial
-- one goes.
CREATE INDEX idx_tasks_job ON tasks (job_id);
DROP INDEX IF EXISTS idx_tasks_job_open;

-- A traject's jobs by the id part of their ref: the last nine characters,
-- `-` and eight hex. The overview matches on that part so a renamed traject
-- keeps its runs, and an index on traject_ref itself cannot serve that
-- expression. The expression and the predicate are the query's own, which is
-- what lets the planner use it.
CREATE INDEX idx_jobs_traject_key ON jobs (right(traject_ref, 9)) WHERE traject_ref IS NOT NULL;
