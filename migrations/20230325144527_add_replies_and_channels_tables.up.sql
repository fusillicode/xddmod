create table npc_replies(
  id integer not null primary key,
  pattern text not null,
  case_insensitive boolean not null default 1,
  template text not null,
  to_mention boolean not null default 0,
  channel text,
  enabled boolean not null default false,
  created_by text not null,
  created_at timestamptz not null default current_timestamp,
  updated_at timestamptz not null default current_timestamp,
  unique (pattern, template, channel)
);

create table channels(
  name text primary key,
  caster text not null,
  date_of_birth date,
  timezone text not null,
  seven_tv_id text,
  created_at timestamptz not null default current_timestamp,
  updated_at timestamptz not null default current_timestamp
);
