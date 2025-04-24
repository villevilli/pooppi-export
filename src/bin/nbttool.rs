use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use futures::executor::block_on;
use poop_scoreboard::stats::Stats;
use sqlx::{Connection, MySqlConnection};

#[derive(Debug, Parser)]
#[command(version,about,long_about= None)]
struct Args {
    #[arg()]
    input_file: PathBuf,
    #[arg(short, long)]
    force: bool,
    #[arg(short, long, group = "output")]
    output_file: Option<PathBuf>,
    #[arg(short, long, group = "output")]
    sql_url: Option<String>,
    #[arg(short, long, requires = "sql_url", value_parser = parse_time)]
    timestamp: Option<DateTime<Utc>>,
}

fn parse_time(arg: &str) -> Result<DateTime<Utc>, String> {
    match DateTime::parse_from_rfc3339(arg) {
        Ok(dt) => Ok(dt.to_utc()),
        Err(e) => Err(e.to_string()),
    }
}

/*
enum Output {
    Sql(SQLOptions),
    Csv(CSVOptions),
}

struct SQLOptions{
    sql_url: String
}

struct CSVOptions{
    output: String
}
 */
fn main() -> Result<()> {
    let args = Args::parse();

    let input_file = fs::File::open(&args.input_file)?;

    if let Some(sql) = args.sql_url {
        write_sql(
            input_file,
            &sql,
            match args.timestamp {
                Some(t) => t,
                None => Utc::now(),
            },
        )?;
    } else {
        let path = match args.output_file {
            Some(path) => path,
            None => args.input_file.with_extension("csv"),
        };

        write_csv(input_file, &path, args.force)?;
    }

    Ok(())
}

fn write_csv(input_file: File, output_file_path: &Path, force: bool) -> Result<()> {
    let stats = Stats::from_gzip_reader(input_file)?;

    let output_file = match force {
        true => fs::File::create(output_file_path)?,
        false => fs::File::create_new(output_file_path).with_context(|| {
            "If you wish to overwrite the existing file, try using the --force flag"
        })?,
    };

    stats.write_csv(output_file)?;

    println!("Converted nbt to csv");

    Ok(())
}

fn write_sql(input_file: File, url: &str, timestamp: DateTime<Utc>) -> Result<()> {
    let mut conn = block_on(MySqlConnection::connect(url))?;

    let stats = Stats::from_gzip_reader(input_file)?;
    block_on(stats.write_to_sql(&mut conn, timestamp)).unwrap();

    Ok(())
}
