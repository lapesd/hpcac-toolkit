-- Add optional_variables column to providers table
ALTER TABLE providers ADD COLUMN optional_variables TEXT NOT NULL DEFAULT '';

-- Set SESSION_TOKEN as optional for AWS
UPDATE providers SET optional_variables = 'SESSION_TOKEN' WHERE id = 'aws';
