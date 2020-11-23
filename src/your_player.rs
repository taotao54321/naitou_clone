//!===================================================================
//! your 側プレイヤー
//!===================================================================

use rand::seq::IteratorRandom;

use crate::prelude::*;
use crate::record::{Record, RecordEntry};
use crate::your_move;

pub trait YourPlayer {
    /// 実装の都合上 pos は &mut にしているが、内容を変更したら元に戻すこと。
    fn think(&mut self, pos: &mut Position) -> Option<Move>;
}

/// 原作で your 側が指せる手から自殺手を除いた手をランダムに指す。
#[derive(Debug)]
pub struct YourPlayerLegal;

impl YourPlayerLegal {
    pub fn new() -> Self {
        Self
    }
}

impl YourPlayer for YourPlayerLegal {
    fn think(&mut self, pos: &mut Position) -> Option<Move> {
        let mut rng = rand::thread_rng();
        your_move::moves_legal(pos).choose(&mut rng)
    }
}

/// 原作で your 側が指せる手(自殺手含む)をランダムに指す。
#[derive(Debug)]
pub struct YourPlayerPseudoLegal;

impl YourPlayerPseudoLegal {
    pub fn new() -> Self {
        Self
    }
}

impl YourPlayer for YourPlayerPseudoLegal {
    fn think(&mut self, pos: &mut Position) -> Option<Move> {
        let mut rng = rand::thread_rng();
        your_move::moves_pseudo_legal(pos).choose(&mut rng)
    }
}

/// 棋譜再現プレイヤー。
#[derive(Debug)]
pub struct YourPlayerRecord {
    record: Record,
}

impl YourPlayerRecord {
    pub fn new(record: Record) -> Self {
        Self { record }
    }
}

impl YourPlayer for YourPlayerRecord {
    fn think(&mut self, pos: &mut Position) -> Option<Move> {
        let your = self.record.handicap().your();
        assert_eq!(pos.side(), your);

        let ply = pos.ply() as usize;
        if ply <= self.record.entrys().len() {
            let entry = &self.record.entrys()[ply - 1];
            if let RecordEntry::Move(mv) = entry {
                Some(mv.clone())
            } else {
                panic!("invalid your move: {}", entry);
            }
        } else {
            None
        }
    }
}
