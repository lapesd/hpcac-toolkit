-- Closes task_runs rows left open by a process that died before it could call
-- TaskRun::finish(). Until now three paths leaked: the watch loop aborting an
-- in-flight run on node failure, Ctrl+C cancelling the task loop, and a hard
-- kill of the controller. Those are handled at the source from now on; this is
-- the one-time backfill for rows that predate the fix.
--
-- finished_at is deliberately left NULL: for these rows we know the run never
-- completed, but not when it stopped, and stamping them 'now' would report a
-- duration of weeks. A NULL duration renders as '-', which is the honest answer.
UPDATE task_runs
SET status = 'orphaned'
WHERE status = 'running' AND finished_at IS NULL;
