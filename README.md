# xddmod

[![Open Source Saturday](https://img.shields.io/badge/%E2%9D%A4%EF%B8%8F-open%20source%20saturday-F64060.svg)](https://www.meetup.com/it-IT/Open-Source-Saturday-Milano/)
[![Top language](https://img.shields.io/github/languages/top/fusillicode/xddmod)](https://www.rust-lang.org/)
[![Commits](https://shields.io/github/last-commit/fusillicode/xddmod)](https://github.com/fusillicode/xddmod/commits/main)
[![Issues](https://shields.io/github/issues/fusillicode/xddmod)](https://github.com/fusillicode/xddmod/issues)
[![Closed issues](https://shields.io/github/issues-closed/fusillicode/xddmod)](https://github.com/fusillicode/xddmod/issues?q=is%3Aissue+is%3Aclosed)

![xdd](https://cdn.7tv.app/emote/641c02da3f88c5f0b445680d/4x.webp)

## Create .env

```
cp .local.env .env
```

## Setup local db

```
cargo sqlx db-reset --database-url sqlite://<LOCAL_DB_FILE>.db
```

## "Prepare" sqlx queries && seed db

```
cargo sqlx prepare --workspace --database-url sqlite://<LOCAL_DB_FILE>.db -- --tests && \
    source .env && cargo run --bin dankcontent -- <LOCAL_DB_FILE>.db <MOD_ID>
```

## Import champions info in db

```
cargo run --bin xtask import-ddragon-champion
    --ddragon-api-base-url http://ddragon.leagueoflegends.com/cdn/<API_VERSION>/data/en_US
    --db-url sqlite://<LOCAL_DB_FILE>.db
```

## Run xddmod

```
source .env && RUST_BACKTRACE=1 cargo run --bin xddmod -- <CHANNEL>
```
