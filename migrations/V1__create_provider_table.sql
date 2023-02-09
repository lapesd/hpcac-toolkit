create table public.providers
(
    uuid             uuid      default gen_random_uuid()
        primary key,
    alias            varchar   not null,
    added_at         timestamp default now(),
    schematics_table varchar   not null
);

alter table public.providers
    owner to local;

create index providers_alias_index
    on public.providers (alias);
