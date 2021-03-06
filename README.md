# 🐦 birdseed 🐦
[![Build
Status](https://travis-ci.org/libellis/birdseed.svg?branch=master)](https://travis-ci.org/libellis/birdseed)
[![crates.io](http://meritbadge.herokuapp.com/birdseed)](https://crates.io/crates/birdseed)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Released API docs](https://docs.rs/birdseed/badge.svg)](https://docs.rs/birdseed)

This crate's entire purpose is to seed the
[libellis](https://github.com/libellis/libellis-backend) database with fake data (generated by
[faker](https://github.com/tikotzky/faker-rs) library).

### Setup

You MUST set a `PSQL_URL` environment variable to your libellis postgres database.

#### Example
```terminal
$ export PSQL_URL=postgres://username:password@localhost/
```

Note the ending forward slash in the example above is **required**. You can optionally set this in a `.env` file in the root folder of this project if you are
running the project from a local folder.

### Installation

You may install this in one of two ways. If you have `cargo` installed then it's very easy. If
not, you can install rust and cargo by following this very simple [cargo setup](https://doc.rust-lang.org/cargo/getting-started/installation.html) process.
Once you have cargo installed you can install this terminal application by running:

```terminal
$ cargo install birdseed
```

Optionally you may instead clone this repo and in the root directory build the release version
of this crate:

```terminal
$ git clone https://github.com/libellis/birdseed.git
$ cd birdseed
$ cargo build --release
```

### Features

#### `setup`

You can setup the main libellis and libellis_test databases with this
subcommand.  It will attempt to drop both libellis and libellis_test before
creating them so be careful! Only use this if you don't need the data in your
libellis database and want to start over, or are creating your libellis databases for the first time.

```terminal
$ birdseed setup
```

#### `rebuild`

You can rebuild all tables according to embedded diesel migrations. This drops each table (but does not drop the database itself) and then rebuilds all tables. Note that you must already have `libellis` and `libellis_test` databases for this to work. If you do not have those databases run `birdseed setup` instead.

```terminal
$ birdseed rebuild
```

`rebuild` by default will rebuild all tables in both your main and test
databases. If you would like to specify to only rebuild one database, pass in
'main' or 'test' to the -database argument flag:

```terminal
$ birdseed rebuild -database main
```

You can also use `-d` for shorthand:

```terminal
$ birdseed rebuild -d test
```
#### `fences`

You can load in fence data from a geojson file with the fences subcommand:

```terminal
$ birdseed fences
```

By default it looks for a file called `fences.json` in the data folder from the root of this
crate. This folder only exists if you cloned the repo.  To specify a filepath yourself pass the
-f or -file flag after the fences subcommand:

```terminal
$ birdseed fences -f BerkeleyNeighborhoods.json
```

Note: This only works if you have a fences table - which would have been setup for you from the
most recent migrations when running `birdseed setup` or `birdseed migrate`.

#### `feed`

You can seed all databases with the `feed` subcommand:

```terminal
$ birdseed feed
```

We can specify a row count (overriding the default of 1000 rows):

```terminal
$ birdseed feed -r 10000
```

In this exampe we override the default of 1,000 rows and instead seed 10,000 rows.

Note: What the row count really means is that we will seed row count amount of users, surveys
and questions, but row count * 4 amount of choices and votes.

#### `migrate`

To run migrations, use the migrate subcommand (this will update your database schema to the
most recent schema).

```terminal
$ birdseed migrate
```

By default this runs migrations on all databases. To run migrations on only main run:

```terminal
$ birdseed migrate -d main
```

To run migrations only on the test database run:

```terminal
$ birdseed migrate -d test
```

#### `clear`

You can clear all tables with the `clear` subcommand:

```terminal
$ birdseed clear
```

#### `icecream`

For fun and profit you can seed the database with an row count amount of users, a single poll
about icecream, and then populate that poll with fake votes from your newly faked user pool,
and have all of their votes counted from legitimate randomized locations within the city of San
Francisco.

```terminal
$ birdseed icecream
```

By default the row count is 1000, and can be overriden in the same way as when using the `feed`
subcommand:

```terminal
$ birdseed icecream -r 10000
```
