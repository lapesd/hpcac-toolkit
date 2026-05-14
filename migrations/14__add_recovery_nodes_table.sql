-- Recovery nodes define the desired cluster topology after a spot interruption.
-- Each row corresponds to a node slot (ordered by insertion) and overrides the
-- original node's instance type and allocation mode on restore.
-- preferred_instance_types is a JSON array ordered by preference (e.g. '["c5.2xlarge","c5.4xlarge"]').
CREATE TABLE recovery_nodes (
    id VARCHAR(32) PRIMARY KEY,
    cluster_id VARCHAR(32) NOT NULL,
    allocation_mode TEXT NOT NULL DEFAULT 'on-demand',
    preferred_instance_types TEXT NOT NULL,
    burstable_mode TEXT NULL,
    image_id TEXT NOT NULL,
    root_volume_gb INTEGER NOT NULL DEFAULT 100,
    root_volume_type TEXT NOT NULL DEFAULT 'gp3',
    root_volume_iops INTEGER NULL,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id)
);
