//!===================================================================
//! 思考ログ
//!
//! 動作検証用。
//!===================================================================

use crate::ai::{BestEval, CandEval, PositionEval, RootEval};
use crate::book::BookState;
use crate::effect::EffectBoard;
use crate::prelude::*;
use crate::record::RecordEntry;

/// 1 候補手に関するログ
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandLog {
    pub mv: Move,
    pub eff_board: EffectBoard,
    pub pos_eval: PositionEval, // 候補手を適用した局面の評価
    pub evals: Vec<CandEval>,   // 評価値が修正されるたびに記録される
    pub improved: bool,         // 最善手を更新したか?
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Log {
    pub progress_ply: u8,
    pub progress_level: u8,
    pub progress_level_sub: u8,
    pub book_state: BookState,

    pub root_eval: RootEval,
    pub root_eff_board: EffectBoard,
    pub cand_logs: Vec<CandLog>,
    pub best_eval: BestEval,
    pub record_entry: RecordEntry,
}

impl Pretty for Log {
    fn pretty(&self) -> std::borrow::Cow<'static, str> {
        use std::fmt::Write;

        let mut res = String::new();

        writeln!(
            res,
            "progress: ply={}, level={}, level_sub={}",
            self.progress_ply, self.progress_level, self.progress_level_sub
        )
        .unwrap();
        writeln!(res, "book_state: {:?}", self.book_state).unwrap();

        writeln!(res, "ルート局面評価: {:?}", self.root_eval).unwrap();
        writeln!(res, "ルート局面利き:").unwrap();
        write!(res, "{}", self.root_eff_board.pretty()).unwrap();

        for cand_log in self.cand_logs.iter() {
            writeln!(
                res,
                "候補手: {}{}",
                cand_log.mv.pretty(),
                if cand_log.improved {
                    " (最善手更新)"
                } else {
                    ""
                }
            )
            .unwrap();

            writeln!(res, "  効き:").unwrap();
            write!(res, "{}", cand_log.eff_board.pretty()).unwrap();

            writeln!(res, "  局面評価: {:?}", cand_log.pos_eval).unwrap();

            for (i, eval) in cand_log.evals.iter().enumerate() {
                writeln!(res, "  評価 {}: {:?}", i, eval).unwrap();
            }
        }

        writeln!(res, "最善手評価: {:?}", self.best_eval).unwrap();
        writeln!(res, "着手: {}", self.record_entry.pretty()).unwrap();

        res.into()
    }
}

pub trait LoggerTrait {
    fn log_progress(&mut self, _ply: u8, _level: u8, _level_sub: u8);
    fn log_book_state(&mut self, _book_state: BookState);

    fn log_root_eval(&mut self, _root_eval: RootEval);
    fn log_root_eff_board(&mut self, _eff_board: EffectBoard);

    fn start_cand(&mut self, _mv: Move);
    fn log_cand_eff_board(&mut self, _eff_board: EffectBoard);
    fn log_cand_pos_eval(&mut self, _pos_eval: PositionEval);
    fn log_cand_eval(&mut self, _cand_eval: CandEval);
    fn log_cand_improve(&mut self);
    fn end_cand(&mut self);

    fn log_best_eval(&mut self, _best_eval: BestEval);
    fn log_record_entry(&mut self, _record_entry: RecordEntry);
}

#[derive(Debug, Default)]
pub struct Logger {
    progress_ply: Option<u8>,
    progress_level: Option<u8>,
    progress_level_sub: Option<u8>,
    book_state: Option<BookState>,

    root_eval: Option<RootEval>,
    root_eff_board: Option<EffectBoard>,
    cand_logs: Vec<CandLog>,
    best_eval: Option<BestEval>,
    record_entry: Option<RecordEntry>,

    cand_mv: Option<Move>,
    cand_eff_board: Option<EffectBoard>,
    cand_pos_eval: Option<PositionEval>,
    cand_evals: Vec<CandEval>,
    cand_improved: bool,
}

impl Logger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_log(self) -> Log {
        assert!(self.progress_ply.is_some());
        assert!(self.progress_level.is_some());
        assert!(self.progress_level_sub.is_some());
        assert!(self.book_state.is_some());

        assert!(self.root_eval.is_some());
        assert!(self.root_eff_board.is_some());
        assert!(self.best_eval.is_some());
        assert!(self.record_entry.is_some());

        Log {
            progress_ply: self.progress_ply.unwrap(),
            progress_level: self.progress_level.unwrap(),
            progress_level_sub: self.progress_level_sub.unwrap(),
            book_state: self.book_state.unwrap(),

            root_eval: self.root_eval.unwrap(),
            root_eff_board: self.root_eff_board.unwrap(),
            cand_logs: self.cand_logs,
            best_eval: self.best_eval.unwrap(),
            record_entry: self.record_entry.unwrap(),
        }
    }
}

impl LoggerTrait for Logger {
    fn log_progress(&mut self, ply: u8, level: u8, level_sub: u8) {
        self.progress_ply = Some(ply);
        self.progress_level = Some(level);
        self.progress_level_sub = Some(level_sub);
    }

    fn log_book_state(&mut self, book_state: BookState) {
        self.book_state = Some(book_state);
    }

    fn log_root_eval(&mut self, root_eval: RootEval) {
        self.root_eval = Some(root_eval);
    }

    fn log_root_eff_board(&mut self, eff_board: EffectBoard) {
        self.root_eff_board = Some(eff_board);
    }

    fn start_cand(&mut self, mv: Move) {
        self.cand_mv = Some(mv);
        self.cand_pos_eval = None;
        self.cand_evals.clear();
        self.cand_improved = false;
    }

    fn log_cand_eff_board(&mut self, eff_board: EffectBoard) {
        self.cand_eff_board = Some(eff_board);
    }

    fn log_cand_pos_eval(&mut self, pos_eval: PositionEval) {
        self.cand_pos_eval = Some(pos_eval);
    }

    fn log_cand_eval(&mut self, cand_eval: CandEval) {
        self.cand_evals.push(cand_eval);
    }

    fn log_cand_improve(&mut self) {
        self.cand_improved = true;
    }

    fn end_cand(&mut self) {
        let cand_log = CandLog {
            mv: self.cand_mv.take().unwrap(),
            eff_board: self.cand_eff_board.take().unwrap(),
            pos_eval: self.cand_pos_eval.take().unwrap(),
            evals: std::mem::replace(&mut self.cand_evals, Vec::new()),
            improved: std::mem::replace(&mut self.cand_improved, false),
        };
        self.cand_logs.push(cand_log);
    }

    fn log_best_eval(&mut self, best_eval: BestEval) {
        self.best_eval = Some(best_eval);
    }

    fn log_record_entry(&mut self, record_entry: RecordEntry) {
        self.record_entry = Some(record_entry);
    }
}

#[derive(Debug)]
pub struct NullLogger;

impl NullLogger {
    pub fn new() -> Self {
        Self
    }
}

impl LoggerTrait for NullLogger {
    fn log_progress(&mut self, _ply: u8, _level: u8, _level_sub: u8) {}
    fn log_book_state(&mut self, _book_state: BookState) {}

    fn log_root_eval(&mut self, _root_eval: RootEval) {}
    fn log_root_eff_board(&mut self, _eff_board: EffectBoard) {}

    fn start_cand(&mut self, _mv: Move) {}
    fn log_cand_eff_board(&mut self, _eff_board: EffectBoard) {}
    fn log_cand_pos_eval(&mut self, _pos_eval: PositionEval) {}
    fn log_cand_eval(&mut self, _cand_eval: CandEval) {}
    fn log_cand_improve(&mut self) {}
    fn end_cand(&mut self) {}

    fn log_best_eval(&mut self, _best_eval: BestEval) {}
    fn log_record_entry(&mut self, _record_entry: RecordEntry) {}
}
