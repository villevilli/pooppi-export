use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug)]
enum Flag {
    JSON,
    CSV,
}

impl FromStr for Flag {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--json" => Ok(Flag::JSON),
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
            'j' => Ok(JSON),
            _ => Err(Error::IncorrecFlags),
        }
    }
}

use poop_scoreboard::{
    error::Error,
    stats::{read_gzip_nbt, write_scores_as_csv},
};

#[derive(Debug)]
struct Args {
    flags: Vec<Flag>,
    input_file: PathBuf,
    output_file: Option<PathBuf>,
}

impl Args {
    fn new() -> Args {
        Args {
            flags: Vec::new(),
            input_file: PathBuf::new(),
            output_file: None,
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
                        args.output_file = Some(std::path::PathBuf::from(&arg_iter.next().unwrap()))
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
    let output_file = fs::File::create(args.get_output_file().with_extension("csv")).unwrap();

    let (objectives, player_scores) = read_gzip_nbt(input_file)?;
    write_scores_as_csv(output_file, player_scores, objectives)?;

    println!(
        "Converted nbt to csv and saved it as {}",
        args.get_output_file().to_str().unwrap()
    );

    Ok(())
}
