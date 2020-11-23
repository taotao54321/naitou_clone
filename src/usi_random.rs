use crate::prelude::*;
use crate::sfen;
use crate::your_move;
use crate::{Error, Result};

const ENGINE_NAME: &str = "naitou_clone_random";
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

fn parse_position_cmd(args: &[&str]) -> Result<Position> {
    let (mut pos, mvs) = sfen::sfen_to_kifu(args.join(" "))?;

    for mv in mvs {
        pos.do_move(&mv)
            .map_err(|e| Error::invalid_usi_cmd(format!("{}", e)))?;
    }

    Ok(pos)
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
        println!("usiok");

        Ok(State::NotReady(StateNotReady::new()))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StateNotReady;

impl StateNotReady {
    fn new() -> Self {
        Self
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "isready" => self.on_cmd_isready(),
            "setoption" => self.on_cmd_setoption(),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_isready(self) -> Result<State> {
        println!("readyok");

        Ok(State::Ready(StateReady::new()))
    }

    fn on_cmd_setoption(self) -> Result<State> {
        // 全て無視する
        Ok(State::NotReady(self))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StateReady;

impl StateReady {
    fn new() -> Self {
        Self
    }

    fn on_cmd(self, cmd: &Cmd) -> Result<State> {
        match cmd.name {
            "quit" => Ok(State::Quit),
            "usinewgame" => self.on_cmd_usinewgame(),
            _ => Err(Error::invalid_usi_cmd(cmd.name)),
        }
    }

    fn on_cmd_usinewgame(self) -> Result<State> {
        Ok(State::WaitingPosition(StateWaitingPosition::new()))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StateWaitingPosition;

impl StateWaitingPosition {
    fn new() -> Self {
        Self
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
        let pos = parse_position_cmd(args)?;
        Ok(State::Playing(StatePlaying::new(pos)))
    }

    fn on_cmd_gameover(self) -> Result<State> {
        Ok(State::NotReady(StateNotReady::new()))
    }
}

/// FIXME: go コマンドのオプションには未対応。
/// 特に infinite を無視してすぐ bestmove を返してしまう。
#[derive(Debug, Eq, PartialEq)]
struct StatePlaying {
    pos: Box<Position>, // State のコピーコストを抑えるため Box に
}

impl StatePlaying {
    fn new(pos: Position) -> Self {
        Self { pos: Box::new(pos) }
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
        use rand::seq::IteratorRandom;
        let mut rng = rand::thread_rng();
        let mv_str = match your_move::moves_legal(&mut self.pos).choose(&mut rng) {
            Some(mv) => sfen::move_to_sfen(&mv),
            None => "resign".into(),
        };
        println!("bestmove {}", mv_str);

        Ok(State::Playing(self))
    }

    fn on_cmd_position(mut self, args: &[&str]) -> Result<State> {
        *self.pos = parse_position_cmd(args)?;
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
