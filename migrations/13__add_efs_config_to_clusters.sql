ALTER TABLE clusters ADD COLUMN efs_performance_mode TEXT NOT NULL DEFAULT 'general_purpose';
ALTER TABLE clusters ADD COLUMN efs_throughput_mode TEXT NOT NULL DEFAULT 'bursting';
ALTER TABLE clusters ADD COLUMN efs_provisioned_throughput_mbs REAL;
