use std::{
    io::{self, Write},
    iter::once,
};

use crate::error::Error;
use nbt::{from_gzip_reader, Blob, Map, Value};
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlQueryResult, query, MySqlConnection};

pub type PlayerScores = Map<String, Vec<PlayerScore>>;
pub type Objectives = Map<String, Objective>;

const PLAYERSCORES: &'static str = "PlayerScores";
const OBJECTIVES: &'static str = "Objectives";

///TODO
#[derive(Debug, Serialize, Deserialize)]
pub struct Stats {
    objectives: Objectives,
    player_scores: PlayerScores,
}

impl Stats {
    pub fn from_gzip_reader(src: impl io::Read) -> Result<Self, Error> {
        Self::parse_blob(from_gzip_reader(src)?)
    }

    fn parse_blob(blob: Blob) -> Result<Self, Error> {
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
                                    player_scores
                                        .insert(key.clone(), vec![player_score.try_into()?]);
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
            _ => panic!("Why is this not a list?"),
        }

        Ok(Stats {
            objectives,
            player_scores,
        })
    }

    pub async fn write_to_sql(&self, conn: &mut MySqlConnection) -> Result<(), Error> {
        let players = self.get_player_list();

        for p in players.iter() {
            query("INSERT IGNORE INTO players (player_name) VALUES (?)")
                .bind(p)
                .execute(&mut *conn)
                .await?;
        }

        for (name, obj) in self.objectives.iter() {
            query(
                "INSERT IGNORE INTO objectives (objective_name, display_name, criteria_name) VALUES (?,?,?);",
            )
            .bind(name)
            .bind(&obj.display_name)
            .bind(&obj.criteria_name)
            .execute(&mut *conn)
            .await?;
        }

        for (obj_name, player_scores) in self.player_scores.iter() {
            for player_score in player_scores {
                query("INSERT INTO stats (score, player_name, objective_name) VALUES (?,?,?)")
                    .bind(&player_score.score)
                    .bind(&player_score.player_name)
                    .bind(&obj_name)
                    .execute(&mut *conn)
                    .await?;
            }
        }

        Ok(())
    }

    pub fn get_player_list(&self) -> Vec<String> {
        let mut players: Vec<String> = self
            .player_scores
            .iter()
            .map(|x| x.1)
            .flatten()
            .map(|x| x.player_name.clone())
            .collect();

        players.sort_unstable();
        players.dedup();

        players
    }

    pub fn write_csv(&self, w: impl Write) -> Result<(), Error> {
        let mut titles: Vec<String> = self.objectives.iter().map(|x| x.0.clone()).collect();
        titles.sort_unstable();

        let mut w = csv::Writer::from_writer(w);

        let mut top_row = vec!["Players".to_string()];

        {
            for i in &titles {
                top_row.push(self.objectives.get(i).unwrap().display_name.clone())
            }
        }

        let players = self.get_player_list();

        w.write_record(top_row)?;

        //loops over every player gathering all the stats
        for player in players {
            let mut row: Vec<String> = Vec::new();

            row.push(player.clone());

            //gathers all the stats for a specific player
            for title in &titles {
                row.push(
                    self.player_scores
                        .get(title)
                        .and_then(|x| {
                            x.iter()
                                .find(|x| x.player_name == player)
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
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Objective {
    criteria_name: String,
    display_auto_update: i8,
    display_name: String,
    render_type: String,
}

impl Objective {
    async fn insert_to_db(
        &self,
        conn: &mut sqlx::MySqlConnection,
    ) -> Result<MySqlQueryResult, sqlx::Error> {
        let query =
            sqlx::query("INSERT INTO objectives (criteria_name, display_name) VALUES (?,?)")
                .bind(&self.criteria_name)
                .bind(&self.display_name);
        query.execute(conn).await
    }
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
    player_name: String,
    score: i64,
}

impl PlayerScore {
    async fn insert_with_name(
        &self,
        conn: &mut sqlx::MySqlConnection,
        obj: Objective,
    ) -> Result<MySqlQueryResult, sqlx::Error> {
        let query =
            sqlx::query("INSERT INTO stats (score,player_name,objective_name) VALUES (?,?,?)")
                .bind(&self.score)
                .bind(&self.player_name)
                .bind(&obj.criteria_name);

        query.execute(conn).await
    }
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
                player_name: {
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
