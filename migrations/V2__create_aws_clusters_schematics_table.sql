create table public.aws_clusters_schematics
(
    uuid           uuid
        primary key
            default gen_random_uuid(),
    alias          varchar not null,
    description    varchar not null,
    az             varchar not null,
    master_ami     varchar not null,
    master_flavor  varchar not null,
    master_ebs     integer not null,
    spot_cluster   boolean not null,
    worker_count   integer not null,
    workers_ami    varchar not null,
    workers_flavor varchar not null,
    workers_ebs    integer not null,
    nfs_support    boolean not null,
    criu_support   boolean not null,
    blcr_support   boolean not null,
    ulfm_support   boolean not null
);
