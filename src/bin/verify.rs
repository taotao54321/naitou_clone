//! 思考ログをエミュレータ上の結果と照合する。

use std::path::{Path, PathBuf};

use chrono::prelude::*;
use structopt::StructOpt;

use naitou_clone::ai::Ai;
use naitou_clone::emu::{self, BTNS_NONE};
use naitou_clone::log::{Log, Logger, LoggerTrait};
use naitou_clone::prelude::*;
use naitou_clone::record::{Record, RecordEntry};
use naitou_clone::your_player::{
    YourPlayer, YourPlayerLegal, YourPlayerPseudoLegal, YourPlayerRecord,
};

const DIR_LOG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/log");

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    path_rom: PathBuf,

    #[structopt(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, StructOpt)]
enum Cmd {
    Legal {
        #[structopt(long)]
        timelimit: bool,
        #[structopt()]
        handicap: Handicap,
    },
    PseudoLegal {
        #[structopt(long)]
        timelimit: bool,
        #[structopt()]
        handicap: Handicap,
    },
    Record {
        #[structopt(parse(from_os_str))]
        path: PathBuf,
    },
}

#[derive(Debug)]
enum VerifyResult {
    Success {
        record: Record,
        logs: Vec<Log>,
    },
    Fail {
        record: Record, // 失敗時の手まで (エミュレータ側)
        logs_ai: Vec<Log>,
        logs_emu: Vec<Log>,
    },
}

/// Rust 側の AI とエミュレータを並行して動かし、思考ログが一致するか検査する。
/// 思考ログが食い違うか、もしくは終局するまで進め、結果を返す。
fn verify<P: YourPlayer>(handicap: Handicap, timelimit: bool, mut player: P) -> VerifyResult {
    let mut ai = Ai::new(handicap, timelimit);

    emu::start_game(handicap, timelimit);
    if ai.is_your_turn() {
        wait_your_turn();
    }

    let my = handicap.my();
    let mut record = Record::new(handicap, timelimit);
    let mut logs_ai = Vec::new();
    let mut logs_emu = Vec::new();

    loop {
        let mut pos = ai.pos().clone();
        println!("{}", pos.pretty());

        // 局面を照合
        // ただし my 側が先手のときの初期局面については実装が面倒なので省略
        if ai.is_your_turn() {
            assert_eq!(pos, emu::get_position());
        }

        // 基本的に your 側の手番を基準として2手1組のループ
        // ただし my 側が先手のときは初手のみスキップ
        let mv_your = if ai.is_your_turn() {
            let mv_your = player.think(&mut pos);
            // your 側が手を返さなかった場合、途中終局とみなす
            if mv_your.is_none() {
                println!("your move: suspend");
                return VerifyResult::Success {
                    record,
                    logs: logs_ai,
                };
            }
            let mv_your = mv_your.unwrap();
            println!("your move: {}", mv_your.pretty());
            record.add(RecordEntry::Move(mv_your.clone()));
            Some(mv_your)
        } else {
            None
        };

        let log_ai = step_ai(&mut ai, &mv_your);
        let log_emu = step_emu(&mv_your, my);
        let ok = log_ai == log_emu;
        let entry = log_ai.record_entry.clone();

        record.add(entry.clone());
        logs_ai.push(log_ai);
        logs_emu.push(log_emu);

        if ok && !matches!(entry, RecordEntry::Move(_)) {
            return VerifyResult::Success {
                record,
                logs: logs_ai,
            };
        }

        if !ok {
            return VerifyResult::Fail {
                record,
                logs_ai,
                logs_emu,
            };
        }
    }
}

fn step_ai(ai: &mut Ai, mv_your: &Option<Move>) -> Log {
    if let Some(mv) = mv_your {
        ai.move_your(&mv);
    }

    let mut logger = Logger::new();
    let record_entry = ai.think(&mut logger);
    match record_entry {
        RecordEntry::Move(mv) => {
            ai.move_my(&mv);
        }
        RecordEntry::MyWin(mv) => {
            ai.move_my(&mv);
        }
        _ => {}
    }

    logger.into_log()
}

fn step_emu(mv_your: &Option<Move>, my: Side) -> Log {
    if let Some(mv) = mv_your {
        emu::move_your(&mv, my.inv());
    }

    let mut logger = Logger::new();
    let mut break_flag = false;

    while !break_flag {
        emu::run_frame_hooked(BTNS_NONE, &|addr: u16| match addr {
            emu::ADDR_YOUR_TURN => {
                break_flag = true;
            }
            emu::ADDR_THINK => {
                logger.log_progress(
                    emu::get_progress_ply(),
                    emu::get_progress_level(),
                    emu::get_progress_level_sub(),
                );
                logger.log_book_state(emu::get_book_state());
                logger.log_root_eff_board(emu::get_effect_board());
            }
            emu::ADDR_ROOT_EVALED => {
                logger.log_root_eval(emu::get_root_eval());
                logger.log_best_eval(emu::get_best_eval()); // デフォルト値
            }
            emu::ADDR_TRY_IMPROVE_BEST => {
                logger.start_cand(emu::get_cand_move());
                logger.log_cand_eff_board(emu::get_effect_board());
                logger.log_cand_pos_eval(emu::get_position_eval());
                logger.log_cand_eval(emu::get_cand_eval());
            }
            emu::ADDR_IMPROVE_BEST => {
                logger.log_cand_improve();
            }
            emu::ADDR_TRY_IMPROVE_BEST_DONE_NONDROP | emu::ADDR_TRY_IMPROVE_BEST_DONE_DROP => {
                logger.end_cand();
            }
            emu::ADDR_THINK_DONE => {
                logger.log_best_eval(emu::get_best_eval());
            }
            emu::ADDR_YOUR_SUICIDE => {
                logger.log_record_entry(RecordEntry::YourSuicide);
                break_flag = true;
            }
            emu::ADDR_YOUR_WIN => {
                logger.log_record_entry(RecordEntry::YourWin);
                break_flag = true;
            }
            emu::ADDR_MOVE_MY => {
                logger.log_record_entry(RecordEntry::Move(emu::get_my_move()));
            }
            emu::ADDR_MY_WIN => {
                logger.log_record_entry(RecordEntry::MyWin(emu::get_my_move()));
                break_flag = true;
            }
            addr if emu::ADDRS_TWEAK.contains(&addr) => {
                logger.log_cand_eval(emu::get_cand_eval());
            }
            _ => {}
        });
    }

    emu::run_frames(3, BTNS_NONE);

    logger.into_log()
}

fn wait_your_turn() {
    let mut your_turn = false;
    while !your_turn {
        emu::run_frame_hooked(BTNS_NONE, &|addr| {
            if addr == emu::ADDR_YOUR_TURN {
                your_turn = true;
            }
        });
    }
}

fn name_datetime() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}

fn save_record(filename: impl AsRef<str>, record: Record) -> eyre::Result<()> {
    let path: PathBuf = [DIR_LOG, filename.as_ref()].iter().collect();

    std::fs::write(path, format!("{}", record))?;

    Ok(())
}

fn save_logs(filename: impl AsRef<str>, logs: Vec<Log>) -> eyre::Result<()> {
    use std::io::Write;

    let path: PathBuf = [DIR_LOG, filename.as_ref()].iter().collect();

    let mut wtr = std::fs::File::create(path)?;
    for log in logs {
        writeln!(wtr, "{}", log.pretty())?;
    }

    Ok(())
}

/// your 側が既存の棋譜を用いないタイプ
/// verify 失敗時、(棋譜, AI思考ログ, emu思考ログ) をログディレクトリに出力する。
fn cmd_nonrecord<P: YourPlayer>(
    handicap: Handicap,
    timelimit: bool,
    player: P,
) -> eyre::Result<()> {
    let res = verify(handicap, timelimit, player);

    if let VerifyResult::Fail {
        record,
        logs_ai,
        logs_emu,
    } = res
    {
        println!("FAILED");
        let stem = name_datetime();
        save_record(format!("{}.record", stem), record)?;
        save_logs(format!("{}.ai.log", stem), logs_ai)?;
        save_logs(format!("{}.emu.log", stem), logs_emu)?;
        std::process::exit(1);
    }

    Ok(())
}

/// your 側が既存の棋譜を用いるタイプ
/// verify 失敗時、(AI思考ログ, emu思考ログ) をログディレクトリに出力する。
fn cmd_record<P: YourPlayer>(
    handicap: Handicap,
    timelimit: bool,
    player: P,
    path: impl AsRef<Path>,
) -> eyre::Result<()> {
    let res = verify(handicap, timelimit, player);

    if let VerifyResult::Fail {
        logs_ai, logs_emu, ..
    } = res
    {
        println!("FAILED");
        let stem = path.as_ref().file_stem().unwrap().to_str().unwrap();
        save_logs(format!("{}.ai.log", stem), logs_ai)?;
        save_logs(format!("{}.emu.log", stem), logs_emu)?;
        std::process::exit(1);
    }

    Ok(())
}

fn main() -> eyre::Result<()> {
    if cfg!(debug_assertions) {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let opt = Opt::from_args();

    emu::init(opt.path_rom)?;

    match opt.cmd {
        Cmd::Legal {
            handicap,
            timelimit,
        } => {
            let player = YourPlayerLegal::new();
            cmd_nonrecord(handicap, timelimit, player)?;
        }

        Cmd::PseudoLegal {
            handicap,
            timelimit,
        } => {
            let player = YourPlayerPseudoLegal::new();
            cmd_nonrecord(handicap, timelimit, player)?;
        }

        Cmd::Record { path } => {
            let record = Record::from_file(&path)?;
            let handicap = record.handicap();
            let timelimit = record.timelimit();
            let player = YourPlayerRecord::new(record);
            cmd_record(handicap, timelimit, player, path)?;
        }
    }

    Ok(())
}
