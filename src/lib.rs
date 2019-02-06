// Licensed under the the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(proc_macro_derive_resolution_fallback)]

//! This crate's entire purpose is to seed the
//! [libellis](https://github.com/libellis/libellis-backend) database with fake data (generated by
//! [faker](https://github.com/tikotzky/faker-rs) library).
//!
//! ## Setup
//!
//! You MUST set a `PSQL_URL` environment variable to your libellis postgres database.
//!
//! ### Example
//! ```terminal
//! $ export DATABASE_URL=postgres://username:password@localhost/
//! ```
//!
//! Note the ending forward slash which is necessary. You can optionally set this in a `.env` file
//! in the root folder of this project if you are running the project from a local folder.
//!
//! ## Installation
//!
//! You may install this in one of two ways. If you have `cargo` installed then it's very easy. If
//! not, you can install rust and cargo by following this very simple [cargo setup](https://doc.rust-lang.org/cargo/getting-started/installation.html) process.
//! Once you have cargo installed you can install this terminal application by running:
//!
//! ```terminal
//! $ cargo install birdseed
//! ```
//!
//! Optionally you may instead clone this repo and in the root directory build the release version
//! of this crate:
//!
//! ```terminal
//! $ git clone https://github.com/libellis/birdseed.git
//! $ cd birdseed
//! $ cargo build --release
//! ```
//!
//! ## Features
//!
//! ### `feed`
//!
//! You can seed all databases with the `feed` subcommand:
//!
//! ```terminal
//! $ birdseed feed
//! ```
//!
//! We can specify a row count (overriding the default of 1000 rows):
//!
//! ```terminal
//! $ birdseed feed -r 10000
//! ```
//!
//! In this exampe we override the default of 1,000 rows and instead seed 10,000 rows.
//!
//! Note: What the row count really means is that we will seed row count amount of users, surveys
//! and questions, but row count * 4 amount of choices and votes.
//!
//! ### `setup`
//!
//! You can setup the main libellis and libellis_test databases with this subcommand.  It will
//! attempt to drop both libellis and libellis_test before creating them so be careful! Only use
//! this if you don't need the data in your libellis database and want to start over, or are
//! creating your libellis databases for the first time.
//!
//! ```terminal
//! $ birdseed setup
//! ```
//!
//! ### `rebuild`
//!
//! You can rebuild all tables according to embedded diesel migrations. This drops each table (but
//! does not drop the database itself) and then rebuilds all tables. Note that you must already
//! have `libellis` and `libellis_test` databases for this to work. If you do not have those
//! databases run `birdseed setup` instead.
//!
//! ```terminal
//! $ birdseed rebuild
//! ```
//!
//! `rebuild` by default will rebuild all tables in both your main and test databases. If you would
//! like to specify to only rebuild one database, pass in 'main' or 'test' to the -database
//! argument flag:
//!
//! ```terminal
//! $ birdseed rebuild -database main
//! ```
//!
//! You can also use `-d` for shorthand:
//!
//! ```terminal
//! $ birdseed rebuild -d test
//! ```
//!
//! ### `migrate`
//!
//! To run migrations, use the migrate subcommand (this will update your database schema to the
//! most recent schema).
//!
//! ```terminal
//! $ birdseed migrate
//! ```
//!
//! By default this runs migrations on all databases. To run migrations on only main run:
//!
//! ```terminal
//! $ birdseed migrate -d main
//! ```
//!
//! To run migrations only on the test database run:
//!
//! ```terminal
//! $ birdseed migrate -d test
//! ```
//!
//! ### `clear`
//!
//! You can clear all tables with the `clear` subcommand:
//!
//! ```terminal
//! $ birdseed clear
//! ```

#[macro_use]
extern crate structopt;

#[macro_use]
extern crate diesel;
extern crate dotenv;

#[macro_use]
extern crate diesel_migrations;

#[macro_use]
extern crate fake;

extern crate indicatif;
extern crate rand;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env;

use std::process::Command;

use std::error::Error;
use std::io;
use std::io::ErrorKind::InvalidInput;
use structopt::StructOpt;

use rand::seq::SliceRandom;
use rand::thread_rng;

use rayon::prelude::*;

use indicatif::{ProgressBar, ProgressStyle};

mod models;
mod schema;
mod pg_pool;

pub use pg_pool::DbConn;
pub use pg_pool::Pool;

embed_migrations!("./migrations");

use self::models::{
    Choice, NewChoice, NewQuestion, NewSurvey, NewUser, NewVote, Question, Survey, User, Vote,
};

/**
 * DEFINED ERRORS
 */

#[derive(StructOpt, Debug)]
#[structopt(
    name = "birdseed",
    about = "The libellis database seeder",
    long_about = "You can use birdseed to seed a libellis db with junk data!"
)]
/// You can use birdseed to seed a libellis db with junk data!
pub enum Birdseed {
    #[structopt(name = "feed")]
    /// Seeds random data into all tables
    Feed {
        /// How many rows to inject
        #[structopt(short = "r", long = "rows", default_value = "1000")]
        row_count: u32,
    },

    #[structopt(name = "setup")]
    /// Builds both libellis main and test databases and runs migrations
    Setup,

    #[structopt(name = "migrate")]
    /// Builds both libellis main and test databases and runs migrations
    Migrate {
        /// Which database to run migrations on, main, test, or all
        #[structopt(short = "d", long = "database", default_value = "all")]
        db: String,
    },

    #[structopt(name = "rebuild")]
    /// Rebuilds all tables per most recent schema (embedded in binary)
    Rebuild {
        /// Which database to run rebuild on, main, test, or all
        #[structopt(short = "d", long = "database", default_value = "all")]
        db: String,
    },

    #[structopt(name = "clear")]
    /// Clears all tables in libellis database
    Clear,
}

/// `run` will take in a Birdseed enum config (parsed in `main`) and either clear all tables or
/// populate all tables with number of rows specified in -r (1000 by default)
pub fn run(config: Birdseed) -> Result<(), Box<dyn Error>> {
    match config {
        Birdseed::Feed { row_count } => populate_all(row_count),
        Birdseed::Rebuild { db } => rebuild(&db),
        Birdseed::Setup => setup(),
        Birdseed::Migrate { db } => migrate(&db),
        Birdseed::Clear => clear_all(),
    }
}

fn setup() -> Result<(), Box<dyn Error>> {
    drop_database("libellis");
    drop_database("libellis_test");
    println!("\r\n                🐦 Creating Main Database 🐦\r\n",);
    setup_database("libellis");
    println!("\r\n                🐦 Creating Test Database 🐦\r\n",);
    setup_database("libellis_test");
    println!("\r\n              🐦 Running Main DB Migrations 🐦\r\n",);
    rebuild("libellis")?;
    println!("\r\n              🐦 Running Test DB Migrations 🐦\r\n",);
    rebuild("libellis_test")?;
    println!("\r\n                        🐦 All Done! 🐦\r\n",);
    Ok(())
}

fn setup_database(database: &str) {
    Command::new("createdb")
        .arg(database)
        .output()
        .expect("failed to create database");
}

fn drop_database(database: &str) {
    Command::new("dropdb")
        .arg(database)
        .output()
        .expect("failed to drop database");
}

// Kicks off populating all tables in main database and updating user
// with visual progress bar along the way
fn populate_all(row_count: u32) -> Result<(), Box<dyn Error>> {
    // get the base url and append it with the db name
    dotenv().ok();
    let base_url = env::var("PSQL_URL")?;
    env::set_var("DATABASE_URL", &format!("{}{}", base_url, "libellis"));

    let pool = generate_pool();
    println!("\r\n                  🐦 Seeding All Tables 🐦\r\n",);

    let bar = ProgressBar::new((row_count * 11) as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {msg}")
            .progress_chars("##-"),
    );

    let usernames = populate_users(&pool, row_count, &bar)?;
    // let survey_ids = populate_surveys(&conn, &usernames, row_count, &bar)?;
    // let question_ids = populate_questions(&conn, &survey_ids, row_count, &bar)?;
    // let choice_ids = populate_choices(&conn, &question_ids, row_count, &bar)?;
    // populate_votes(&conn, &usernames, &choice_ids, &bar)?;
    bar.finish();
    println!("\r\n             🐦 Birdseed has Finished Seeding! 🐦\r\n",);
    Ok(())
}

fn migrate(database: &str) -> Result<(), Box<dyn Error>> {
    // get the base url and append it with the db name
    dotenv().ok();
    let base_url = env::var("PSQL_URL")?;

    match database {
        "all" | "a" => {
            migrate_main(&base_url)?;
            migrate_test(&base_url)?;
        }
        "main" | "m" => migrate_main(&base_url)?,
        "test" | "t" => migrate_test(&base_url)?,
        _ => {
            return Err(io::Error::new(
                InvalidInput,
                "Invalid Database Type, choose 'main', 'test', or 'all'",
            )
            .into());
        }
    };

    Ok(())
}

fn migrate_main(base_url: &String) -> Result<(), Box<dyn Error>> {
    env::set_var("DATABASE_URL", &format!("{}{}", base_url, "libellis"));

    let conn = establish_connection();

    println!("\r\n                  🐦 Running Migrations on Main DB 🐦\r\n");
    embedded_migrations::run_with_output(&conn, &mut std::io::stdout())?;

    Ok(())
}

fn migrate_test(base_url: &String) -> Result<(), Box<dyn Error>> {
    env::set_var("DATABASE_URL", &format!("{}{}", base_url, "libellis_test"));

    let conn = establish_connection();

    println!("\r\n                  🐦 Running Migrations on Test DB 🐦\r\n");
    embedded_migrations::run_with_output(&conn, &mut std::io::stdout())?;

    Ok(())
}

// Clears all tables in appropriate order and increments a progress bar with
// custom start and completion messages
fn clear_all() -> Result<(), Box<dyn Error>> {
    use self::schema::*;

    // get the base url and append it with the db name
    dotenv().ok();
    let base_url = env::var("PSQL_URL")?;
    std::env::set_var("DATABASE_URL", &format!("{}{}", base_url, "libellis"));

    let conn = establish_connection();

    println!("\r\n                  🐦 Clearing all Tables 🐦\r\n");

    let bar = ProgressBar::new(5);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {msg}")
            .progress_chars("##-"),
    );

    diesel::delete(votes::table).execute(&conn)?;
    bar.inc(1);
    diesel::delete(choices::table).execute(&conn)?;
    bar.inc(1);
    diesel::delete(questions::table).execute(&conn)?;
    bar.inc(1);
    diesel::delete(surveys::table).execute(&conn)?;
    bar.inc(1);
    diesel::delete(users::table).execute(&conn)?;
    bar.inc(1);

    bar.finish();
    println!("\r\n                  🐦 All Tables Cleared! 🐦\r\n");

    Ok(())
}

// Drops all tables from libellis database
// Note that it's very intentional that we're not dealing with the result ->
// We don't care if a table doesn't exist, we simply want to attempt to drop
// ALL regardless so we are ready for a rebuild.  This takes care of situations
// where a user has manually deleted some but not all of their tables. Open a PR
// request if you have a better solution in mind.
fn drop_all(conn: &PgConnection) {
    let bar = ProgressBar::new(7);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {msg}")
            .progress_chars("##-"),
    );

    conn.execute("DROP VIEW users_votes");
    bar.inc(1);
    conn.execute("DROP TABLE votes");
    bar.inc(1);
    conn.execute("DROP TABLE choices");
    bar.inc(1);
    conn.execute("DROP TABLE questions");
    bar.inc(1);
    conn.execute("DROP TABLE surveys");
    bar.inc(1);
    conn.execute("DROP TABLE users");
    bar.inc(1);
    conn.execute("DROP TABLE __diesel_schema_migrations");
    bar.inc(1);
    bar.finish();
}

// Rebuilds all tables per most recent embedded diesel migrations
fn rebuild(database: &str) -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let base_url = env::var("PSQL_URL")?;

    match database {
        "all" | "a" => {
            rebuild_main(&base_url)?;
            rebuild_test(&base_url)?;
        }
        "main" | "m" => rebuild_main(&base_url)?,
        "test" | "t" => rebuild_test(&base_url)?,
        _ => {
            return Err(io::Error::new(
                InvalidInput,
                "Invalid Database Type, choose 'main', 'test', or 'all'",
            )
            .into());
        }
    };

    Ok(())
}

fn rebuild_main(base_url: &str) -> Result<(), Box<dyn Error>> {
    std::env::set_var("DATABASE_URL", &format!("{}{}", base_url, "libellis"));
    println!("\r\n                🐦 Connecting to Main DB 🐦\r\n");
    let conn = establish_connection();
    println!("\r\n                 🐦 Dropping all Tables 🐦\r\n");
    drop_all(&conn);
    println!("\r\n                  🐦 Running Migrations 🐦\r\n");
    embedded_migrations::run_with_output(&conn, &mut std::io::stdout())?;
    println!("\r\n              🐦 Tables Successfully Rebuilt! 🐦\r\n");
    Ok(())
}

fn rebuild_test(base_url: &str) -> Result<(), Box<dyn Error>> {
    std::env::set_var("DATABASE_URL", &format!("{}{}", base_url, "libellis_test"));
    println!("\r\n                🐦 Connecting to Test DB 🐦\r\n");
    let conn = establish_connection();
    println!("\r\n                 🐦 Dropping all Tables 🐦\r\n");
    drop_all(&conn);
    println!("\r\n                  🐦 Running Migrations 🐦\r\n");
    embedded_migrations::run_with_output(&conn, &mut std::io::stdout())?;
    println!("\r\n              🐦 Tables Successfully Rebuilt! 🐦\r\n");
    Ok(())
}

// Populates users table with row_count random users
fn populate_users(
    pool: &Pool,
    row_count: u32,
    bar: &ProgressBar,
) -> Result<Vec<String>, Box<dyn Error>> {
    bar.set_message(&format!("Seeding {} users", row_count));

    let usernames: Vec<String> = (0..row_count).into_par_iter().map(|_| {
        let pool = pool.clone();

        let conn = pool.get().unwrap();

        let user = format!(
            "{}{}",
            fake!(Internet.user_name),
            fake!(Number.between(90, 9999))
        );
        let pw = format!(
            "{}{}",
            fake!(Name.name),
            fake!(Number.between(10000, 99999))
        );
        let em = format!("{}@gmail.com", user);
        let first = format!("{}", fake!(Name.first_name));
        let last = format!("{}", fake!(Name.last_name));

        create_user(&conn, &user, &pw, &em, &first, &last);
        bar.inc(1);

		user
    }).collect();

    Ok(usernames)
}

// Populates surveys table with row_count random surveys making sure each survey is being created
// by an existing user
fn populate_surveys(
    conn: &PgConnection,
    authors: &Vec<String>,
    row_count: u32,
    bar: &ProgressBar,
) -> Result<Vec<i32>, Box<dyn Error>> {
    let mut survey_ids = Vec::new();
    bar.set_message(&format!("Seeding {} surveys", row_count));
    for i in 0..row_count as usize {
        let auth = &authors[i];
        let survey_title = format!("{}", fake!(Lorem.sentence(4, 8)));
        let survey = create_survey(conn, auth, &survey_title);
        survey_ids.push(survey.id);
        bar.inc(1);
    }

    Ok(survey_ids)
}

// Populates questions table with row_count random questions ensuring that each question relates to
// an existing survey
fn populate_questions(
    conn: &PgConnection,
    survey_ids: &Vec<i32>,
    row_count: u32,
    bar: &ProgressBar,
) -> Result<Vec<i32>, Box<dyn Error>> {
    let mut question_ids = Vec::new();
    bar.set_message(&format!("Seeding {} questions", row_count));
    for i in 0..row_count as usize {
        let s_id = survey_ids[i];
        let q_title = format!("{}", fake!(Lorem.sentence(3, 7)));
        let q_type = "multiple".to_string();
        let question = create_question(conn, s_id, &q_type, &q_title);
        question_ids.push(question.id);
        bar.inc(1);
    }

    Ok(question_ids)
}

// Populates choices table with row_count * 4 random choices ensuring that each question relates to
// an existing survey
fn populate_choices(
    conn: &PgConnection,
    question_ids: &Vec<i32>,
    row_count: u32,
    bar: &ProgressBar,
) -> Result<Vec<i32>, Box<dyn Error>> {
    let mut choice_ids = Vec::new();
    bar.set_message(&format!("Seeding {} choices", (row_count * 4)));
    for i in 0..row_count as usize {
        let q_id = question_ids[i];
        // For each question, inject 4 random text choices
        for _ in 0..4 {
            let c_title = format!("{}", fake!(Lorem.sentence(1, 4)));
            let c_type = "text".to_string();
            let choice = create_choice(conn, q_id, &c_type, &c_title);
            choice_ids.push(choice.id);
            bar.inc(1);
        }
    }

    Ok(choice_ids)
}

// Populates the votes table with real votes from our newly inserted random users who vote on
// choices in a semi-randomish way (not that random really)
fn populate_votes(
    conn: &PgConnection,
    authors: &Vec<String>,
    choice_ids: &Vec<i32>,
    bar: &ProgressBar,
) -> Result<(), Box<dyn Error>> {
    bar.set_message(&format!("{} users are voting", (authors.len())));

    // Create vectors of idx and shuffle them
    let mut rng = thread_rng();
    let mut choice_idxs: Vec<usize> = (0..choice_ids.len()).collect();
    let choice_slice: &mut [usize] = &mut choice_idxs;
    let mut author_idxs: Vec<usize> = (0..authors.len()).collect();
    let author_slice: &mut [usize] = &mut author_idxs;
    choice_slice.shuffle(&mut rng);
    author_slice.shuffle(&mut rng);

    // For each round up randomize the choice and the author voting
    // on the choice
    for i in 0..authors.len() - 1 {
        let name = &authors[author_slice[i]];
        for j in 1..=4 {
            let c_id = choice_ids[choice_slice[(i + 1) * j]];
            create_vote(conn, c_id, name, 1);
            bar.inc(1);
        }
    }

    Ok(())
}

// Establishes a connection to the libellis postgres database on your machine, as specified by your
// DATABASE_URL environment variable. Returns a single PgConnection
fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

// Establishes a connection to the libellis postgres database on your machine, as specified by your
// DATABASE_URL environment variable. Returns a Pool
fn generate_pool() -> Pool {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    pg_pool::init(&database_url)
}

/**
 * The following series of functions are very simple - each one simply creates a single
 * user/survey/question/choice/vote
 */
fn create_user<'a>(
    conn: &PgConnection,
    user: &'a str,
    pw: &'a str,
    em: &'a str,
    first: &'a str,
    last: &'a str,
) -> User {
    use self::schema::users;

    let new_user = NewUser {
        username: user,
        password: pw,
        email: em,
        first_name: first,
        last_name: last,
        is_admin: false,
    };

    diesel::insert_into(users::table)
        .values(&new_user)
        .get_result(conn)
        .expect("Error saving new user")
}

fn create_survey<'a>(conn: &PgConnection, auth: &'a str, survey_title: &'a str) -> Survey {
    use self::schema::surveys;

    let new_survey = NewSurvey {
        author: auth,
        title: survey_title,
        published: true,
    };

    diesel::insert_into(surveys::table)
        .values(&new_survey)
        .get_result(conn)
        .expect("Error saving new survey")
}

fn create_question<'a>(
    conn: &PgConnection,
    s_id: i32,
    q_type: &'a str,
    q_title: &'a str,
) -> Question {
    use self::schema::questions;

    let new_question = NewQuestion {
        survey_id: s_id,
        question_type: q_type,
        title: q_title,
    };

    diesel::insert_into(questions::table)
        .values(&new_question)
        .get_result(conn)
        .expect("Error saving new question")
}

fn create_choice<'a>(conn: &PgConnection, q_id: i32, c_type: &'a str, c_title: &'a str) -> Choice {
    use self::schema::choices;

    let new_choice = NewChoice {
        question_id: q_id,
        content_type: c_type,
        title: c_title,
    };

    diesel::insert_into(choices::table)
        .values(&new_choice)
        .get_result(conn)
        .expect("Error saving new choice")
}

fn create_vote<'a>(conn: &PgConnection, c_id: i32, name: &'a str, points: i32) -> Vote {
    use self::schema::votes;

    let new_vote = NewVote {
        choice_id: c_id,
        username: name,
        score: points,
    };

    diesel::insert_into(votes::table)
        .values(&new_vote)
        .get_result(conn)
        .expect("Error saving new vote")
}
