-- no-transaction
-- A run that a member of its traject stopped ends as `cancelled`, not as
-- `failed`: nothing went wrong, and a failed run would get a retry and a
-- "Verrijking mislukt" task. ALTER TYPE ... ADD VALUE cannot run inside a
-- transaction block, hence the no-transaction marker (same pattern as
-- 0031_traject_harvest_job_type.sql). The column that records the request
-- follows in 0040, in a migration of its own.
ALTER TYPE job_status ADD VALUE IF NOT EXISTS 'cancelled';
