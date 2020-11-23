//! 最短勝利手順を求める

use arrayvec::ArrayVec;
use itertools::Itertools;
use rayon::prelude::*;
use structopt::StructOpt;

use naitou_clone::ai::Ai;
use naitou_clone::log::NullLogger;
use naitou_clone::prelude::*;
use naitou_clone::record::RecordEntry;
use naitou_clone::sfen;
use naitou_clone::your_move;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(long)]
    timelimit: bool,

    #[structopt()]
    handicap: Handicap,

    #[structopt()]
    depth: i32,
}

/// ai は your 側の手番と仮定している。
fn rec(sols: &mut Vec<Vec<Move>>, ai: &mut Ai, history: &mut Vec<Move>, depth: i32) {
    if depth <= 0 {
        return;
    }

    let mvs_your: ArrayVec<[Move; 1024]> = your_move::moves_pseudo_legal(ai.pos()).collect();

    for mv_your in mvs_your {
        let cmd_your = ai.move_your(&mv_your);
        history.push(mv_your);

        let (entry, step_my_cmd) = ai.step_my(&mut NullLogger::new());
        match entry {
            RecordEntry::Move(mv_my) => {
                history.push(mv_my);

                rec(sols, ai, history, depth - 1);

                history.pop().unwrap();
            }
            RecordEntry::YourWin => {
                sols.push(history.clone());
            }
            RecordEntry::MyWin(_) | RecordEntry::YourSuicide => {}
        }
        ai.undo_step_my(&step_my_cmd);

        history.pop().unwrap();
        ai.undo_move_your(&cmd_your);
    }
}

fn step(ai: &mut Ai, history: &mut Vec<Move>, mv_your: &Move) {
    ai.move_your(mv_your);
    history.push(mv_your.clone());

    let (entry, _) = ai.step_my(&mut NullLogger::new());
    match entry {
        RecordEntry::Move(mv_my) => {
            history.push(mv_my);
        }
        _ => panic!("unexpected: {}", entry),
    }
}

fn solve(mut ai: Ai, mut history: Vec<Move>, mv_your: &Move, depth: i32) -> Vec<Vec<Move>> {
    step(&mut ai, &mut history, mv_your);

    let mut sols = Vec::new();
    rec(&mut sols, &mut ai, &mut history, depth - 1);

    sols
}

fn main() -> eyre::Result<()> {
    let opt = Opt::from_args();

    let mut ai = Ai::new(opt.handicap, opt.timelimit);
    let mut history = Vec::new();

    if ai.is_my_turn() {
        let (entry, _) = ai.step_my(&mut NullLogger::new());
        match entry {
            RecordEntry::Move(mv_my) => {
                history.push(mv_my);
            }
            _ => panic!("unexpected: {}", entry),
        }
    }

    // TAS 手順 (平手、your 先手、時間制限なし)
    //step(&mut ai, &mut history, &Move::from_sfen("5g5f").unwrap());
    //step(&mut ai, &mut history, &Move::from_sfen("2h5h").unwrap());
    //step(&mut ai, &mut history, &Move::from_sfen("7g7f").unwrap());
    //step(&mut ai, &mut history, &Move::from_sfen("8h5e").unwrap());
    //step(&mut ai, &mut history, &Move::from_sfen("5e6f").unwrap());
    //step(&mut ai, &mut history, &Move::from_sfen("5f5e").unwrap());

    let mvs_your: ArrayVec<[Move; 1024]> = your_move::moves_pseudo_legal(ai.pos()).collect();

    let sols: Vec<_> = mvs_your
        .par_iter()
        .flat_map(|mv_your| solve(ai.clone(), history.clone(), mv_your, opt.depth))
        .collect();

    for sol in sols {
        println!("{}", sol.iter().map(|mv| sfen::move_to_sfen(mv)).join(" "));
    }

    Ok(())
}
