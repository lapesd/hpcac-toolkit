create table public.aws_azs
(
    code varchar primary key
);

alter table public.aws_azs
    owner to local;

insert into aws_azs (code) values ('us-east-1a');
insert into aws_azs (code) values ('us-east-1b');
insert into aws_azs (code) values ('us-east-1c');
