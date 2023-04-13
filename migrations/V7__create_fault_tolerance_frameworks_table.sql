create table public.fault_tolerance_frameworks
(
    uuid           uuid
        primary key
            default gen_random_uuid(),
    alias varchar not null,
    description varchar not null,
    install_script_path varchar not null
);

alter table public.fault_tolerance_frameworks
    owner to local;
