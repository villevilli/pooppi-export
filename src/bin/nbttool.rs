use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug)]
enum Flag {
    SQL,
    CSV,
}

impl FromStr for Flag {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--sql" => Ok(Flag::SQL),
            "--csv" => Ok(Flag::CSV),
            _ => Err(Error::IncorrecFlags),
        }
    }
}

impl TryFrom<char> for Flag {
    type Error = Error;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        use self::Flag::*;

        match c {
            'c' => Ok(CSV),
            's' => Ok(SQL),
            _ => Err(Error::IncorrecFlags),
        }
    }
}

use futures::executor::block_on;
use poop_scoreboard::{error::Error, stats::Stats};
use sqlx::{Connection, MySqlConnection};

#[derive(Debug)]
struct Args {
    flags: Vec<Flag>,
    input_file: PathBuf,
    output_file: Option<PathBuf>,
    sql_url: Option<String>,
}

impl Args {
    fn new() -> Args {
        Args {
            flags: Vec::new(),
            input_file: PathBuf::new(),
            output_file: None,
            sql_url: None,
        }
    }

    fn get_output_file(&self) -> &Path {
        match &self.output_file {
            Some(s) => s.as_path(),
            None => &self.input_file.as_path(),
        }
    }
}

impl TryFrom<Vec<String>> for Args {
    type Error = Error;

    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        let mut arg_iter = value.into_iter();

        let mut args = Args::new();

        //skips the first argument that is usually the binary
        arg_iter.next();

        while let Some(s) = arg_iter.next() {
            if s.starts_with("--") {
                if let Some(flag) = s.parse().ok() {
                    args.flags.push(flag);
                }
            } else if s.starts_with("-") {
                s.chars().for_each(|char| {
                    if let Some(flag) = char.try_into().ok() {
                        args.flags.push(flag)
                    }
                })
            } else {
                match arg_iter.len() {
                    1 => {
                        args.input_file = std::path::PathBuf::from(&s);
                        let second = &arg_iter.next().unwrap();

                        match args.flags[0] {
                            Flag::CSV => args.output_file = Some(std::path::PathBuf::from(second)),
                            Flag::SQL => args.sql_url = Some(second.clone()),
                        }
                    }
                    0 => args.input_file = std::path::PathBuf::from(&s),
                    _ => return Err(Error::IncorrecFlags),
                }
            }
        }

        Ok(args)
    }
}

fn main() -> Result<(), Error> {
    let args: Args = match std::env::args().collect::<Vec<String>>().try_into() {
        Ok(x) => x,
        Err(x) => panic!("{}", x),
    };

    let input_file = fs::File::open(&args.input_file).unwrap();

    match args.flags[0] {
        Flag::SQL => write_sql(input_file, &args.sql_url.unwrap()),
        Flag::CSV => {
            let output_file =
                fs::File::create(args.get_output_file().with_extension("csv")).unwrap();
            write_csv(input_file, output_file)
        }
    }?;

    Ok(())
}

fn write_csv(input_file: File, output_file: File) -> Result<(), Error> {
    let stats = Stats::from_gzip_reader(input_file)?;
    stats.write_csv(output_file)?;

    println!("Converted nbt to csv");

    Ok(())
}

fn write_sql(input_file: File, url: &str) -> Result<(), Error> {
    let mut conn = block_on(MySqlConnection::connect(url))?;

    let stats = Stats::from_gzip_reader(input_file)?;
    block_on(stats.write_to_sql(&mut conn)).unwrap();

    Ok(())
}
