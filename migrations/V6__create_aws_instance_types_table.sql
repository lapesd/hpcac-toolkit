create table public.aws_instance_types
(
    uuid           uuid
        primary key
            default gen_random_uuid(),
    instance_family varchar not null,
    instance_size varchar not null,
    vcpus integer not null,
    memory float not null,
    on_demand_linux_pricing decimal not null
);

alter table public.aws_instance_types
    owner to local;

insert into aws_instance_types (instance_family, instance_size, vcpus, memory, on_demand_linux_pricing) values 
    ('t1', 'micro',   1,  0.612, '0.02'),
    ('t2', 'nano',    1,  0.5,   '0.0058'),
    ('t2', 'micro',   1,  1,     '0.0116'),
    ('t2', 'small',   1,  2,     '0.023'),
    ('t2', 'medium',  2,  4,     '0.0464'),
    ('t2', 'large',   2,  8,     '0.0928'),
    ('t2', 'xlarge',  4,  16,    '0.1856'),
    ('t2', '2xlarge', 8,  32,    '0.3712'),
    ('t3', 'nano',    2,  0.5,   '0.0052'),
    ('t3', 'micro',   2,  1,     '0.0104'),
    ('t3', 'small',   2,  2,     '0.0208'),
    ('t3', 'medium',  2,  4,     '0.0416'),
    ('t3', 'large',   2,  8,     '0.0832'),
    ('t3', 'xlarge',  4,  16,    '0.1664'),
    ('t3', '2xlarge', 8,  32,    '0.3328');
