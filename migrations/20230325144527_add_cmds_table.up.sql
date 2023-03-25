create table cmds(
  id integer primary key autoincrement,
  shortcut text not null,
  expansion text not null,
  created_by text not null,
  created_at timestamptz not null default current_timestamp
);
