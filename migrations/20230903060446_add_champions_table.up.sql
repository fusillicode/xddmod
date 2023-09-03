create table champions(
    version text not null,
    id text not null,
    "key" text primary key,
    name text not null,
    title text not null,
    blurb text not null,
    info json not null,
    image json not null,
    tags json not null,
    partype text not null,
    stats json not null
)