-- Migration: Drop status column and add EFS/SSH configuration tracking columns

ALTER TABLE nodes DROP COLUMN status;
ALTER TABLE nodes ADD COLUMN was_efs_configured BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE nodes ADD COLUMN was_ssh_configured BOOLEAN NOT NULL DEFAULT false;
