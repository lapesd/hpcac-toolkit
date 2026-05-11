-- Add optional provisioned IOPS for root EBS volumes.
-- Supported for gp3 (up to 16000, baseline 3000), io1 (up to 64000), io2 (up to 256000).
-- NULL means use the volume type's default (3000 for gp3, not applicable for gp2).
ALTER TABLE nodes ADD COLUMN root_volume_iops INTEGER;
