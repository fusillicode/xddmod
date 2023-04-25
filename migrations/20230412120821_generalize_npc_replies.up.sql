drop table npc_replies;

create table replies(
  id integer not null primary key,
  handler text,
  pattern text not null,
  case_insensitive boolean not null default 1,
  template text not null,
  to_mention boolean not null default 0,
  channel text,
  enabled boolean not null default false,
  created_by text not null,
  created_at timestamptz not null default current_timestamp,
  updated_at timestamptz not null default current_timestamp,
  unique (handler, pattern, template, channel)
);
