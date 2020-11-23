//!===================================================================
//! 棋譜
//!===================================================================

use std::path::Path;

use itertools::Itertools;

use crate::prelude::*;
use crate::sfen;
use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordEntry {
    Move(Move),
    MyWin(Move),
    YourSuicide,
    YourWin,
}

impl std::fmt::Display for RecordEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Move(mv) => write!(f, "{}", sfen::move_to_sfen(&mv)),
            Self::MyWin(mv) => write!(f, "!{}", sfen::move_to_sfen(&mv)),
            Self::YourSuicide => write!(f, "YourSuicide"),
            Self::YourWin => write!(f, "YourWin"),
        }
    }
}

impl std::str::FromStr for RecordEntry {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "YourSuicide" => Ok(Self::YourSuicide),
            "YourWin" => Ok(Self::YourWin),
            _ => {
                if let Some(sfen_mv) = s.strip_prefix('!') {
                    let mv = sfen::sfen_to_move(sfen_mv)
                        .map_err(|e| Error::record_parse_error(e.to_string()))?;
                    Ok(Self::MyWin(mv))
                } else {
                    let mv = sfen::sfen_to_move(s)
                        .map_err(|e| Error::record_parse_error(e.to_string()))?;
                    Ok(Self::Move(mv))
                }
            }
        }
    }
}

impl Pretty for RecordEntry {
    fn pretty(&self) -> std::borrow::Cow<'static, str> {
        match self {
            Self::Move(mv) => mv.pretty(),
            Self::MyWin(mv) => format!("{} (わたしの勝ち)", mv.pretty()).into(),
            Self::YourSuicide => "わたしの勝ち".into(),
            Self::YourWin => "あなたの勝ち".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    handicap: Handicap,
    timelimit: bool,
    entrys: Vec<RecordEntry>,
}

impl Record {
    pub fn new(handicap: Handicap, timelimit: bool) -> Self {
        Self {
            handicap,
            timelimit,
            entrys: Vec::new(),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let s =
            std::fs::read_to_string(path).map_err(|e| Error::record_parse_error(e.to_string()))?;
        s.parse::<Self>()
    }

    pub fn handicap(&self) -> Handicap {
        self.handicap
    }

    pub fn timelimit(&self) -> bool {
        self.timelimit
    }

    pub fn entrys(&self) -> &[RecordEntry] {
        &self.entrys
    }

    pub fn add(&mut self, entry: RecordEntry) {
        self.entrys.push(entry);
    }
}

impl std::fmt::Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.handicap)?;
        writeln!(f, "{}", self.timelimit)?;

        let pos_str = self.handicap.initial_pos().to_sfen();
        let entrys_str = self
            .entrys
            .iter()
            .map(|entry| format!("{}", entry))
            .join(" ");
        writeln!(f, "{} moves {}", pos_str, entrys_str)?;

        Ok(())
    }
}

impl std::str::FromStr for Record {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut lines = s.lines();
        let mut next = || {
            lines
                .next()
                .ok_or_else(|| Error::record_parse_error("incomplete"))
        };

        let handicap = next()?
            .parse::<Handicap>()
            .map_err(|e| Error::record_parse_error(e.to_string()))?;
        let timelimit = next()?
            .parse::<bool>()
            .map_err(|e| Error::record_parse_error(e.to_string()))?;

        let entrys = {
            let line = next()?;
            let mut it = line.split_ascii_whitespace();
            let magic = it
                .next()
                .ok_or_else(|| Error::record_parse_error("magic not found"))?;
            let pos_str = match magic {
                "startpos" => "startpos".to_owned(),
                "sfen" => ["sfen", &it.by_ref().take(4).join(" ")].join(" "),
                _ => {
                    return Err(Error::record_parse_error(format!(
                        "invalid magic: {}",
                        magic
                    )))
                }
            };
            let pos = sfen::sfen_to_position(pos_str)
                .map_err(|e| Error::record_parse_error(e.to_string()))?;
            if pos != handicap.initial_pos() {
                return Err(Error::record_parse_error("initial position mismatch"));
            }
            if !it.next().map_or(false, |s| s == "moves") {
                return Err(Error::record_parse_error("moves not found"));
            }
            it.map(|s| s.parse::<RecordEntry>())
                .collect::<Result<Vec<_>>>()?
        };

        Ok(Self {
            handicap,
            timelimit,
            entrys,
        })
    }
}
