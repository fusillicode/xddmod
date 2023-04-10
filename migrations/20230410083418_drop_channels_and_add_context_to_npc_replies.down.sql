create table channels(
  name text primary key,
  caster text not null,
  date_of_birth date,
  timezone text not null,
  seven_tv_id text,
  created_at timestamptz not null default current_timestamp,
  updated_at timestamptz not null default current_timestamp
);

alter table npc_replies drop column context;
