-- Add cost tracking fields to clusters.
-- cost_per_hour: total estimated cost per hour for all nodes (instances + EBS + public IPs).
-- cost_breakdown: JSON with per-node itemized costs.
ALTER TABLE clusters ADD COLUMN cost_per_hour REAL NOT NULL DEFAULT 0.0;
ALTER TABLE clusters ADD COLUMN cost_breakdown TEXT NOT NULL DEFAULT '{}';
