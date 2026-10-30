ALTER TABLE clusters ADD COLUMN use_fsx_lustre BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE clusters ADD COLUMN fsx_deployment_type TEXT NOT NULL DEFAULT 'scratch_2';
ALTER TABLE clusters ADD COLUMN fsx_storage_capacity_gib INTEGER NOT NULL DEFAULT 1200;
ALTER TABLE clusters ADD COLUMN fsx_per_unit_storage_throughput INTEGER;
