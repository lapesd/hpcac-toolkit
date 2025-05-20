-- Migration to add 'node_affinity' column to the 'clusters' table
ALTER TABLE clusters
ADD COLUMN node_affinity BOOLEAN NOT NULL DEFAULT FALSE;
