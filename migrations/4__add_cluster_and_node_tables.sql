-- Create the CLUSTERS table
CREATE TABLE clusters (
    id VARCHAR(32) PRIMARY KEY,
    display_name TEXT NOT NULL UNIQUE,
    provider_id VARCHAR(32) NOT NULL,
    provider_config_id INTEGER NOT NULL,
    public_ssh_key_path TEXT NOT NULL,
    private_ssh_key_path TEXT NOT NULL,
    region TEXT NOT NULL,
    availability_zone TEXT NOT NULL,
    use_node_affinity BOOLEAN NOT NULL,
    use_elastic_fabric_adapters BOOLEAN NOT NULL,
    use_elastic_file_system BOOLEAN NOT NULL,
    created_at DATETIME NOT NULL,
    state TEXT NOT NULL,
    on_instance_creation_failure TEXT,
    migration_attempts INTEGER,
    tried_zones TEXT,
    FOREIGN KEY (provider_config_id) REFERENCES provider_configs(id),
    FOREIGN KEY (provider_id) REFERENCES providers(id)
);

-- Create the NODES table
CREATE TABLE nodes (
    id VARCHAR(32) PRIMARY KEY,
    cluster_id VARCHAR(32) NOT NULL,
    status TEXT NOT NULL,
    instance_type TEXT NOT NULL,
    allocation_mode TEXT NOT NULL,
    burstable_mode TEXT NULL,
    image_id TEXT NOT NULL,
    private_ip TEXT NULL,
    public_ip TEXT NULL,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id)
);
