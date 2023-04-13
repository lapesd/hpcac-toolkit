create table public.aws_amis
(
    code varchar primary key,
    alias varchar not null
);

alter table public.aws_amis
    owner to local;

insert into aws_amis (code, alias) values ('ami-08e4e35cccc6189f4', 'Amazon Linux 2');
