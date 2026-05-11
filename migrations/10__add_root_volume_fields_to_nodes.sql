-- Add root EBS volume configuration to nodes.
-- Supported types for root volumes: gp2, gp3, io1, io2 (HDD types not boot-eligible).
ALTER TABLE nodes ADD COLUMN root_volume_gb INTEGER NOT NULL DEFAULT 100;
ALTER TABLE nodes ADD COLUMN root_volume_type TEXT NOT NULL DEFAULT 'gp3';
