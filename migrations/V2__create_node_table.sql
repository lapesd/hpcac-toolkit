create table public.node
(
    uuid       uuid      not null
        constraint node_pk
            primary key,
    owner      uuid      not null
        constraint owner_fk
            references public.cluster
            on update restrict on delete restrict,
    public_ip  varchar,
    private_ip varchar   not null,
    flavor     varchar   not null,
    status     varchar,
    created_at timestamp not null,
    updated_at timestamp
);

alter table public.node
    owner to local;
