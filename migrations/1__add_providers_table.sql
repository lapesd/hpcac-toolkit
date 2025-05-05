-- Create the PROVIDERS table
CREATE TABLE providers (
    id VARCHAR(32) PRIMARY KEY,
    display_name VARCHAR(50) NOT NULL,
    required_variables TEXT NOT NULL,
    supports_spot BOOLEAN NOT NULL
);

-- Insert base PROVIDERS records
INSERT INTO providers (id, display_name, required_variables, supports_spot) VALUES 
    ('aws', 'Amazon Web Services', 'ACCESS_KEY_ID,SECRET_ACCESS_KEY', 1),
    ('vultr', 'Vultr Cloud', 'API_KEY', 0);
