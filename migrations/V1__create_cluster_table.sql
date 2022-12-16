create table public.cluster
(
    uuid            uuid      not null
        constraint cluster_pk
            primary key,
    provider        varchar   not null,
    created_at      timestamp not null,
    name            varchar   not null,
    status          integer,
    entrypoint_uuid uuid
);

alter table public.cluster
    owner to local;

create index cluster_name_index
    on public.cluster (name);
