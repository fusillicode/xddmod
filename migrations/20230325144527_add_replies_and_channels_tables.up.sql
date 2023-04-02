create table replies(
  id integer not null primary key,
  pattern text not null,
  case_insensitive boolean not null default 1,
  expansion text not null,
  to_mention boolean not null default 0,
  channel text,
  enabled boolean not null default false,
  created_by text not null,
  created_at timestamptz not null default current_timestamp,
  updated_at timestamptz not null default current_timestamp,
  unique (pattern, expansion, channel)
);

create table channels(
  name text primary key,
  caster text not null,
  emotes_7tv_id text,
  timezone text not null,
  created_at timestamptz not null default current_timestamp,
  updated_at timestamptz not null default current_timestamp
);
