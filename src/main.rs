use std::{
    collections::{hash_map, linked_list::Iter},
    fmt::Display,
    fs,
    hash::Hash,
    io::{self, Write},
    path::{self, Path, PathBuf},
    str::FromStr,
};

const PLAYERSCORES: &'static str = "PlayerScores";
const OBJECTIVES: &'static str = "Objectives";

use nbt::{from_gzip_reader, Blob, Map, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[allow(dead_code)]
enum Error {
    NBTError(nbt::Error),
    IOError(io::Error),
    SerdeJsonError(serde_json::Error),
    CSVError(csv::Error),
    NBTMissingField(&'static str),
    IncorrecFlags,
    LOLError,
    NOTLOLError,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            Self::NBTMissingField(_) => None,
            Self::IncorrecFlags => None,
            Self::LOLError => None,
            Self::NOTLOLError => None,
            &error => error.source(),
        }
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::NBTMissingField(missing_field) => {
                write!(
                    f,
                    "NBT data file is missing the required field \"{}\"",
                    missing_field
                )
            }
            Self::IncorrecFlags => write!(f, "IncorrecFlags"),
            Self::LOLError => write!(f, "LOLError"),
            Self::NOTLOLError => write!(f, "NOTLOLError"),
            &error => {
                write!(f, "{}", error.to_string())
            }
        }
    }
}

impl From<nbt::Error> for Error {
    fn from(value: nbt::Error) -> Self {
        Self::NBTError(value)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeJsonError(value)
    }
}

impl From<csv::Error> for Error {
    fn from(value: csv::Error) -> Self {
        Self::CSVError(value)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Objective {
    criteria_name: String,
    display_auto_update: i8,
    display_name: String,
    render_type: String,
}

impl TryFrom<&Value> for Objective {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        use Error::LOLError;

        match value {
            Value::Compound(val) => Ok(Self {
                criteria_name: {
                    match val.get("CriteriaName").ok_or(LOLError)? {
                        Value::String(s) => Ok(s.clone()),
                        _ => Err(LOLError),
                    }?
                },
                display_auto_update: {
                    match val.get("display_auto_update").ok_or(LOLError)? {
                        Value::Byte(s) => Ok(s.clone()),
                        _ => Err(LOLError),
                    }?
                },
                display_name: {
                    match val.get("DisplayName").ok_or(LOLError)? {
                        Value::String(s) => {
                            let mut chars = s.chars();
                            chars.next();
                            chars.next_back();
                            Ok(chars.as_str().to_string())
                        }
                        _ => Err(LOLError),
                    }?
                },
                render_type: {
                    match val.get("RenderType").ok_or(LOLError)? {
                        Value::String(s) => Ok(s.clone()),
                        _ => Err(LOLError),
                    }?
                },
            }),
            _ => Err(LOLError),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PlayerScore {
    locked: i8,
    name: String,
    score: i64,
}

impl TryFrom<&Value> for PlayerScore {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        use Error::{LOLError, NOTLOLError};

        match value {
            Value::Compound(val) => Ok(Self {
                locked: {
                    match val.get("Locked").ok_or(NOTLOLError)? {
                        Value::Byte(s) => Ok(s.clone()),
                        _ => Err(LOLError),
                    }?
                },
                name: {
                    match val.get("Name").ok_or(NOTLOLError)? {
                        Value::String(s) => Ok(s.clone()),
                        _ => Err(NOTLOLError),
                    }?
                },
                score: {
                    match val.get("Score").ok_or(NOTLOLError)? {
                        Value::Long(s) => Ok(*s),
                        Value::Int(s) => Ok(*s as i64),
                        Value::Short(s) => Ok(*s as i64),
                        Value::Byte(s) => Ok(*s as i64),
                        _ => Err(NOTLOLError),
                    }?
                },
            }),
            _ => Err(NOTLOLError),
        }
    }
}

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

    dbg!(&args);

    let input_file = fs::File::open(&args.input_file).unwrap();

    //Deserialize the data
    let binding: Blob = from_gzip_reader(input_file)?;

    let data: &nbt::Value = binding.get("data").ok_or(Error::NBTMissingField("data"))?;

    let mut objectives: Map<String, Objective> = Map::new();

    let raw_objectives = match data {
        Value::Compound(x) => x
            .get(OBJECTIVES)
            .ok_or(Error::NBTMissingField(OBJECTIVES))?,
        _ => panic!(),
    };

    match raw_objectives {
        Value::List(raw_objectives) => {
            for objective in raw_objectives {
                match objective {
                    nbt::Value::Compound(objective_map) => {
                        let key = &objective_map.get("Name").unwrap().to_string();

                        objectives.insert(key.clone(), objective.try_into()?);
                    }
                    _ => (),
                }
            }
        }
        _ => panic!("Why is this not a list?"),
    }

    let mut player_scores: Map<String, Vec<PlayerScore>> = Map::new();

    let raw_player_scores = match data {
        Value::Compound(x) => x
            .get(PLAYERSCORES)
            .ok_or(Error::NBTMissingField(PLAYERSCORES))?,
        _ => panic!(),
    };

    match raw_player_scores {
        Value::List(raw_player_scores) => {
            for player_score in raw_player_scores {
                match player_score {
                    Value::Compound(player_scores_map) => {
                        let key = &player_scores_map.get("Objective").unwrap().to_string();

                        match player_scores.contains_key(key) {
                            true => player_scores
                                .get_mut(key)
                                .unwrap()
                                .push(player_score.try_into()?),
                            false => {
                                player_scores.insert(key.clone(), vec![player_score.try_into()?]);
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        _ => panic!("Why is this not a list?"),
    }

    let output_file = fs::File::create(args.get_output_file().with_extension("csv")).unwrap();

    let mut output_file = csv::Writer::from_writer(output_file);

    let mut titles: Vec<String> = objectives.iter().map(|x| x.0.clone()).collect();
    titles.sort_unstable();

    let mut players: Vec<String> = player_scores
        .iter()
        .map(|x| x.1)
        .flatten()
        .map(|x| x.name.clone())
        .collect();

    players.sort_unstable();
    players.dedup();

    let mut top_row = vec!["Players".to_string()];

    {
        for i in &titles {
            top_row.push(objectives.get(i).unwrap().display_name.clone())
        }
    }

    output_file.write_record(top_row)?;

    for player in players {
        let mut row: Vec<String> = Vec::new();

        row.push(player.clone());

        for title in &titles {
            row.push(
                player_scores
                    .get(title)
                    .and_then(|x| {
                        x.iter()
                            .find(|x| x.name == player)
                            .and_then(|x| Some(x.score.to_string()))
                    })
                    .unwrap_or(String::from("0")),
            );
        }
        output_file.write_record(row)?;
    }

    output_file.flush()?;

    println!(
        "Converted nbt to csv and saved it as {}",
        args.get_output_file().to_str().unwrap()
    );

    Ok(())
}

fn read_nbt() {
    todo!()
}
