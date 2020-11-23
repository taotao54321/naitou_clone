use crate::ai::Ai;
use crate::log::NullLogger;
use crate::prelude::*;
use crate::record::RecordEntry;
use crate::sfen;
use crate::{Error, Result};

const ENGINE_NAME: &str = "naitou_clone";
const ENGINE_AUTHOR: &str = "TaoTao";

#[derive(Debug, Eq, PartialEq)]
struct Cmd<'a> {
    name: &'a str,
    args: &'a [&'a str],
}

impl<'a> Cmd<'a> {
    fn new(name: &'a str, args: &'a [&'a str]) -> Self {
        Self { name, args }
    }
}

fn get_handicap(pos: &Position, my: Side) -> Option<Handicap> {
    let pos_hirate = Position::from_sfen(sfen::SFEN_HIRATE).unwrap();
    let pos_hishaochi = Position::from_sfen(sfen::SFEN_HISHAOCHI).unwrap();
    let pos_nimaiochi = Position::from_sfen(sfen::SFEN_NIMAIOCHI).unwrap();

    if *pos == pos_hirate {
        match my {
            Side::Sente => Some(Handicap::MySente),
            Side::Gote => Some(Handicap::YourSente),
        }
    } else if *pos == pos_hishaochi {
        match my {
            Side::Sente => Some(Handicap::MyHishaochi),
            Side::Gote => Some(Handicap::YourHishaochi),
        }
    } else if *pos == pos_nimaiochi {
        match my {
            Side::Sente => Some(Handicap::MyNimaiochi),
            Side::Gote => Some(Handicap::YourNimaiochi),
        }
    } else {
        None
    }
}

/// sfen に書かれている ply は無視する。
fn parse_position_cmd(args: &[&str], timelimit: bool) -> Result<Ai> {
    let (pos, mvs) = sfen::sfen_to_kifu(args.join(" "))?;

    // 現局面が AI の手番とみなす
    let my = if mvs.len() % 2 == 0 {
        Side::Sente
    } else {
        Side::Gote
    };

    let handicap =
        get_handicap(&pos, my).ok_or_else(|| Error::invalid_usi_cmd("unsupported handicap"))?;

    let mut ai = Ai::new(handicap, timelimit);

    // mvs を再生し、現局面まで進める
    // AI 側の手は一致するものと仮定する
    for mv in mvs {
        if ai.pos().side() == my {
            let mut logger = NullLogger::new();
            match ai.think(&mut logger) {
                RecordEntry::Move(mv_actual) => {
                    if mv != mv_actual {
                        return Err(Error::invalid_usi_cmd(format!(
                            "move mismatch (sfen: {:?}, actual: {:?}",
                            mv, mv_actual
                        )));
                    }
                    ai.move_my(&mv);
                }
                RecordEntry::MyWin(mv_actual) => {
                    if mv != mv_actual {
                        return Err(Error::invalid_usi_cmd(format!(
                            "move mismatch (sfen: {:?}, actual: {:?}",
                            mv, mv_actual
                        )));
                    }
                    ai.move_my(&mv);
                }
                RecordEntry::YourSuicide => {
                    return Err(Error::invalid_usi_cmd(format!(
                        "move mismatch (sfen: {:?}, actual: YourSuicide",
                        mv
                    )));
                }
                RecordEntry::YourWin => {
                    return Err(Error::invalid_usi_cmd(format!(
                        "move mismatch (sfen: {:?}, actual: YourWin",
                        mv
                    )));
                }
            }
        } else {
            ai.move_your(&mv);
        }
    }

    Ok(ai)
}

#[derive(Debug, Eq, PartialEq)]
struct StateInitial;

impl StateInitial {
    fn new() -> Self {
        Self
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "usi" => self.on_cmd_usi(),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_usi(self) -> Result<State> {
        println!("id name {}", ENGINE_NAME);
        println!("id author {}", ENGINE_AUTHOR);
        println!("option name timelimit type check default false");
        println!("usiok");

        Ok(State::NotReady(StateNotReady::new()))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StateNotReady {
    timelimit: bool,
}

impl StateNotReady {
    fn new() -> Self {
        Self { timelimit: false }
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "isready" => self.on_cmd_isready(),
            "setoption" => self.on_cmd_setoption(cmd.args),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_isready(self) -> Result<State> {
        println!("readyok");

        Ok(State::Ready(StateReady::new(self.timelimit)))
    }

    /// name timelimit value <true|false> のみ対応。
    fn on_cmd_setoption(mut self, args: &[&str]) -> Result<State> {
        if args.len() != 4 {
            return Ok(State::NotReady(self));
        }

        chk!(
            args[0] == "name",
            Error::invalid_usi_cmd("\"name\" expected")
        );
        chk!(
            args[2] == "value",
            Error::invalid_usi_cmd("\"value\" expected")
        );

        let name = args[1];
        let value = args[3];

        if name != "timelimit" {
            return Ok(State::NotReady(self));
        }

        self.timelimit = value
            .parse()
            .map_err(|e| Error::invalid_usi_cmd(format!("bool parse error: {}", e)))?;

        Ok(State::NotReady(self))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StateReady {
    timelimit: bool,
}

impl StateReady {
    fn new(timelimit: bool) -> Self {
        Self { timelimit }
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "usinewgame" => self.on_cmd_usinewgame(),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_usinewgame(self) -> Result<State> {
        Ok(State::WaitingPosition(StateWaitingPosition::new(
            self.timelimit,
        )))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StateWaitingPosition {
    timelimit: bool,
}

impl StateWaitingPosition {
    fn new(timelimit: bool) -> Self {
        Self { timelimit }
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "position" => self.on_cmd_position(cmd.args),
            "gameover" => self.on_cmd_gameover(),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_position(self, args: &[&str]) -> Result<State> {
        let ai = parse_position_cmd(args, self.timelimit)?;
        Ok(State::Playing(StatePlaying::new(self.timelimit, ai)))
    }

    fn on_cmd_gameover(self) -> Result<State> {
        Ok(State::NotReady(StateNotReady::new()))
    }
}

/// FIXME: go コマンドのオプションには未対応。
/// 特に infinite を無視してすぐ bestmove を返してしまう。
#[derive(Debug, Eq, PartialEq)]
struct StatePlaying {
    timelimit: bool,
    ai: Box<Ai>, // State のコピーコストを抑えるため Box に
}

impl StatePlaying {
    fn new(timelimit: bool, ai: Ai) -> Self {
        Self {
            timelimit,
            ai: Box::new(ai),
        }
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "go" => self.on_cmd_go(cmd.args),
            "position" => self.on_cmd_position(cmd.args),
            "stop" => self.on_cmd_stop(),
            "gameover" => self.on_cmd_gameover(),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_go(mut self, _args: &[&str]) -> Result<State> {
        let mut logger = NullLogger::new();
        let mv_str = match self.ai.think(&mut logger) {
            RecordEntry::Move(mv) => Ok(sfen::move_to_sfen(&mv)),
            RecordEntry::MyWin(mv) => Ok(sfen::move_to_sfen(&mv)),
            RecordEntry::YourSuicide => Err(Error::invalid_usi_cmd("YourSuicide")),
            RecordEntry::YourWin => Ok("resign".into()),
        }?;
        println!("bestmove {}", mv_str);

        Ok(State::Playing(self))
    }

    fn on_cmd_position(mut self, args: &[&str]) -> Result<State> {
        *self.ai = parse_position_cmd(args, self.timelimit)?;
        Ok(State::Playing(self))
    }

    fn on_cmd_stop(self) -> Result<State> {
        // FIXME: go infinite 未対応なので、とりあえず単に無視
        Ok(State::Playing(self))
    }

    fn on_cmd_gameover(self) -> Result<State> {
        Ok(State::NotReady(StateNotReady::new()))
    }
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    Quit,
    Initial(StateInitial),
    NotReady(StateNotReady),
    Ready(StateReady),
    WaitingPosition(StateWaitingPosition),
    Playing(StatePlaying),
}

impl State {
    fn new() -> Self {
        Self::Initial(StateInitial::new())
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<Self> {
        match self {
            Self::Quit => unreachable!(),
            Self::Initial(st) => st.on_cmd(cmd),
            Self::NotReady(st) => st.on_cmd(cmd),
            Self::Ready(st) => st.on_cmd(cmd),
            Self::WaitingPosition(st) => st.on_cmd(cmd),
            Self::Playing(st) => st.on_cmd(cmd),
        }
    }
}

pub fn interact() -> Result<()> {
    use std::io::{self, BufRead};

    let stdin = io::stdin();
    let stdin = stdin.lock();
    let rdr = io::BufReader::new(stdin);

    let mut state = State::new();
    for line in rdr.lines() {
        let line = line?;
        let mut it = line.split_ascii_whitespace();

        // 空行は無視する
        if let Some(name) = it.next() {
            let args: Vec<_> = it.collect();
            let cmd = Cmd::new(name, &args);
            eprintln!("{:?}", cmd);
            state = state.on_cmd(&cmd)?;
        }

        if matches!(state, State::Quit) {
            break;
        }
    }

    Ok(())
}
