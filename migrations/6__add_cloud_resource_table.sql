-- Create the CLOUD_RESOURCES table
CREATE TABLE cloud_resources (
    id VARCHAR(255) PRIMARY KEY,
    cluster_id VARCHAR(32) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    provider VARCHAR(50) NOT NULL,
    region VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id)
);

-- Create an index for faster lookups by cluster_id
CREATE INDEX idx_cloud_resources_cluster_id ON cloud_resources(cluster_id);

-- Create an index for faster lookups by provider and region
CREATE INDEX idx_cloud_resources_provider_region ON cloud_resources(provider, region);

-- Create the RESOURCE_TAGS table for storing resource tags
CREATE TABLE resource_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    resource_id VARCHAR(255) NOT NULL,
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    FOREIGN KEY (resource_id) REFERENCES cloud_resources(id) ON DELETE CASCADE
);

-- Create an index for faster lookups of tags by resource_id
CREATE INDEX idx_resource_tags_resource_id ON resource_tags(resource_id);

-- Create a unique index to prevent duplicate tags for the same resource
CREATE UNIQUE INDEX idx_resource_tags_unique ON resource_tags(resource_id, key);
