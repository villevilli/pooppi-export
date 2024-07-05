use std::io;

use crate::error::Error;
use nbt::{from_gzip_reader, Blob, Map, Value};
use serde::{Deserialize, Serialize};

pub type PlayerScores = Map<String, Vec<PlayerScore>>;
pub type Objectives = Map<String, Objective>;

const PLAYERSCORES: &'static str = "PlayerScores";
const OBJECTIVES: &'static str = "Objectives";

#[derive(Debug, Serialize, Deserialize)]
pub struct Objective {
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
pub struct PlayerScore {
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

pub fn read_gzip_nbt(src: impl io::Read) -> Result<(Objectives, PlayerScores), Error> {
    read_nbt_blob(from_gzip_reader(src)?)
}

fn read_nbt_blob(blob: Blob) -> Result<(Objectives, PlayerScores), Error> {
    let data: &nbt::Value = blob.get("data").ok_or(Error::NBTMissingField("data"))?;

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

    Ok((objectives, player_scores))
}

pub fn write_scores_as_csv(
    w: impl io::Write,
    player_scores: PlayerScores,
    objectives: Objectives,
) -> Result<(), Error> {
    let mut titles: Vec<String> = objectives.iter().map(|x| x.0.clone()).collect();
    titles.sort_unstable();

    let mut w = csv::Writer::from_writer(w);

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

    w.write_record(top_row)?;

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
        w.write_record(row)?;
    }

    w.flush()?;

    Ok(())
}
