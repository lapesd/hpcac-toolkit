-- Create the PROVIDER_CONFIGS table
CREATE TABLE provider_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id VARCHAR(32) NOT NULL,
    display_name VARCHAR(50) NOT NULL UNIQUE,
    FOREIGN KEY (provider_id) REFERENCES providers(id)
);

-- Create the CONFIG_VARIABLES table
CREATE TABLE config_variables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_config_id INTEGER NOT NULL,
    key VARCHAR(128) NOT NULL,
    value VARCHAR(128) NOT NULL,
    FOREIGN KEY (provider_config_id) REFERENCES provider_configs(id)
);
