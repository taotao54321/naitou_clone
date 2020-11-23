//!===================================================================
//! 利き処理全般
//!
//! 駒の利きは相対インデックスで表現される。
//!
//! 「近接利き」とは、「その方向に 1 回だけ進める利き」の意。
//! 桂の利きや馬の縦横の利きなどはこれに含まれる。
//!
//! 「遠隔利き」とは、「その方向に何回でも進める利き」の意。
//! 馬の斜めの効きなどはこれに含まれる。
//!
//! 駒の利きの列挙順は思考ルーチンの挙動に影響することに注意。
//!===================================================================

use crate::prelude::*;
use crate::price::PRICES_0;
use crate::util;

//--------------------------------------------------------------------
// 駒の利き
//--------------------------------------------------------------------

/// (side, pt) の近接利きを返す。
pub fn piece_effects_melee(side: Side, pt: Piece) -> impl Iterator<Item = i32> {
    const EFFECTS_GOLD: &[i32] = &[-12, -11, -10, -1, 1, 11];

    let effects: &[i32] = match pt {
        Piece::Pawn => &[-11],
        Piece::Knight => &[-23, -21],
        Piece::Silver => &[-12, -11, -10, 10, 12],
        Piece::King => &[-12, -11, -10, -1, 1, 10, 11, 12],
        Piece::Horse => &[-11, -1, 1, 11],
        Piece::Dragon => &[-12, -10, 10, 12],
        Piece::Gold | Piece::ProPawn | Piece::ProLance | Piece::ProKnight | Piece::ProSilver => {
            EFFECTS_GOLD
        }
        _ => &[], // 香、角、飛
    };

    effects.iter().map(move |di| di * side.sgn())
}

/// (side, pt) の遠隔利きを返す。
pub fn piece_effects_ranged(side: Side, pt: Piece) -> impl Iterator<Item = i32> {
    let effects: &[i32] = match pt {
        Piece::Lance => &[-11],
        Piece::Bishop | Piece::Horse => &[-12, -10, 10, 12],
        Piece::Rook | Piece::Dragon => &[-11, 11, -1, 1],
        _ => &[], // 近接駒
    };

    effects.iter().map(move |di| di * side.sgn())
}

//--------------------------------------------------------------------
// 盤上の利き
//--------------------------------------------------------------------

/// board 上の side 側の利きを列挙する。
///
/// イテレータの要素は (src, dst)
pub fn iter_effects(board: &Board, side: Side) -> impl Iterator<Item = (Sq, Sq)> + '_ {
    Sq::iter_valid()
        .filter_map(move |src| {
            board[src]
                .piece_of(side)
                .map(move |pt| iter_effects_by(board, side, src, pt).map(move |dst| (src, dst)))
        })
        .flatten()
}

/// board 上の駒 (side, sq, pt) による利きを列挙する。
/// 実際にこの駒があるかどうかは気にしない。
fn iter_effects_by(board: &Board, side: Side, sq: Sq, pt: Piece) -> impl Iterator<Item = Sq> + '_ {
    itertools::chain(
        iter_melee_effects_by(side, sq, pt),
        iter_ranged_effects_by(board, side, sq, pt),
    )
}

/// board 上の駒 (side, sq, pt) による近接利きを列挙する。
/// 実際にこの駒があるかどうかは気にしない。
fn iter_melee_effects_by(side: Side, sq: Sq, pt: Piece) -> impl Iterator<Item = Sq> {
    // valid なマスしか返されないはず(入力に1段目の桂が現れないと仮定すれば)
    pt.effects_melee(side)
        .map(move |di| sq + di)
        .filter(Sq::is_valid)
}

/// board 上の駒 (side, sq, pt) による遠隔利きを列挙する。
/// 実際にこの駒があるかどうかは気にしない。
fn iter_ranged_effects_by(
    board: &Board,
    side: Side,
    sq: Sq,
    pt: Piece,
) -> impl Iterator<Item = Sq> + '_ {
    pt.effects_ranged(side)
        .flat_map(move |dir| iter_uni_ranged_effects_by(board, sq, dir))
}

/// board 上で sq から dir 方向への遠隔利きを列挙する。
/// sq に駒があるかどうかは気にしない。
fn iter_uni_ranged_effects_by(board: &Board, sq: Sq, dir: i32) -> impl Iterator<Item = Sq> + '_ {
    let mut dst = sq + dir;
    let it = move || {
        let nxt = dst + dir;
        std::mem::replace(&mut dst, nxt)
    };

    std::iter::repeat_with(it).scan(true, move |ok, dst| {
        if !*ok {
            return None;
        }
        if board[dst].is_wall() {
            return None;
        }
        if !board[dst].is_empty() {
            *ok = false;
        }
        Some(dst)
    })
}

/// board 上の side 側の利きを列挙する。(影の利き対応)
/// 原作では my 側の手番によってマスの列挙順が変わるため、my 引数が必要。
///
/// イテレータの要素は (影の利きかどうか, src, dst)
/// 影の利きは利き数は増やすが attacker には影響しないため、区別が必要。
/// よって安直だがフラグを付加して解決。
fn iter_support_effects(
    board: &Board,
    side: Side,
    my: Side,
) -> impl Iterator<Item = (bool, Sq, Sq)> + '_ {
    Sq::iter_valid_sim(my)
        .filter_map(move |src| {
            board[src].piece_of(side).map(move |pt| {
                iter_support_effects_by(board, side, src, pt)
                    .map(move |(is_support, dst)| (is_support, src, dst))
            })
        })
        .flatten()
}

/// board 上の駒 (side, sq, pt) による利きを列挙する。(影の利き対応)
/// 実際にこの駒があるかどうかは気にしない。
fn iter_support_effects_by(
    board: &Board,
    side: Side,
    sq: Sq,
    pt: Piece,
) -> impl Iterator<Item = (bool, Sq)> + '_ {
    itertools::chain(
        iter_melee_support_effects_by(side, sq, pt),
        iter_ranged_support_effects_by(board, side, sq, pt),
    )
}

/// board 上の駒 (side, sq, pt) による近接利きを列挙する。(影の利き対応)
/// 近接利きは影の利きにならないので is_support は常に false となる。
/// 実際にこの駒があるかどうかは気にしない。
fn iter_melee_support_effects_by(
    side: Side,
    sq: Sq,
    pt: Piece,
) -> impl Iterator<Item = (bool, Sq)> {
    iter_melee_effects_by(side, sq, pt).map(|dst| (false, dst))
}

/// board 上の駒 (side, sq, pt) による遠隔利きを列挙する。(影の利き対応)
/// 実際にこの駒があるかどうかは気にしない。
fn iter_ranged_support_effects_by(
    board: &Board,
    side: Side,
    sq: Sq,
    pt: Piece,
) -> impl Iterator<Item = (bool, Sq)> + '_ {
    pt.effects_ranged(side)
        .flat_map(move |dir| iter_uni_ranged_support_effects_by(board, side, sq, dir))
}

/// board 上で side 側の sq から dir 方向への遠隔利きを列挙する。(影の利き対応)
/// sq に駒があるかどうかは気にしない。
fn iter_uni_ranged_support_effects_by(
    board: &Board,
    side: Side,
    sq: Sq,
    dir: i32,
) -> impl Iterator<Item = (bool, Sq)> + '_ {
    let mut dst = sq + dir;
    let it = move || {
        let nxt = dst + dir;
        std::mem::replace(&mut dst, nxt)
    };

    enum State {
        Normal,
        Support,
        Break,
    }

    std::iter::repeat_with(it).scan(State::Normal, move |state, dst| match state {
        State::Normal => {
            let cell = board[dst];
            if cell.is_wall() {
                return None;
            }
            // 味方駒に当たったら影の利きを処理
            if let Some(pt) = cell.piece_of(side) {
                *state = if can_support(side, dir, pt) {
                    State::Support
                } else {
                    State::Break
                };
            } else if cell.is_piece() {
                *state = State::Break;
            }
            Some((false, dst))
        }
        State::Support => {
            if board[dst].is_wall() {
                return None;
            }
            *state = State::Break;
            Some((true, dst))
        }
        State::Break => None,
    })
}

/// side 側の dir 方向の遠隔利きが駒 pt 上にあるとき影の利きが生じるかどうかを返す。
fn can_support(side: Side, dir: i32, pt: Piece) -> bool {
    if matches!(pt, Piece::King) {
        return false;
    }

    // pt が dir 方向への利きを持つことが条件
    pt.effects_melee(side).any(|e| e == dir) || pt.effects_ranged(side).any(|e| e == dir)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectInfo {
    count: u8,
    attacker: Option<Piece>,
}

impl EffectInfo {
    pub fn new(count: u8, attacker: Option<Piece>) -> Self {
        Self { count, attacker }
    }

    pub fn count(&self) -> u8 {
        self.count
    }

    pub fn attacker(&self) -> Option<Piece> {
        self.attacker
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectBoardCell([EffectInfo; 2]);

impl std::ops::Index<Side> for EffectBoardCell {
    type Output = EffectInfo;

    fn index(&self, side: Side) -> &Self::Output {
        &self.0[side]
    }
}

impl std::ops::IndexMut<Side> for EffectBoardCell {
    fn index_mut(&mut self, side: Side) -> &mut Self::Output {
        &mut self.0[side]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectBoard {
    cells: [EffectBoardCell; 11 * 11],
}

impl EffectBoard {
    pub fn empty() -> Self {
        let cells =
            array_init::from_iter((0..11 * 11).map(|_| EffectBoardCell::default())).unwrap();

        Self { cells }
    }

    /// board 上の利き計算を行う。
    /// attacker が複数ある場合、駒価値の低いものを優先する。
    /// 駒価値が同点ならば先に設定された attacker が優先される。
    /// よって、利きの列挙順が計算結果に影響することに注意。
    ///
    /// 原作の仕様に合わせるため my 引数が必要。
    pub fn from_board(board: &Board, my: Side) -> Self {
        let mut this = Self::empty();

        for side in Side::iter() {
            for (is_support, src, dst) in iter_support_effects(board, side, my) {
                let info = &mut this[dst][side];

                info.count += 1;

                // 影の利きでなければ attacker 更新処理
                // 既存の attacker がある場合、駒価値が (新規) < (既存) なら更新
                if !is_support {
                    let pt = board[src].piece_of(side).unwrap();
                    util::opt_chmin_by_key(&mut info.attacker, pt, |&p| PRICES_0[p]);
                }
            }
        }

        this
    }
}

impl std::ops::Index<Sq> for EffectBoard {
    type Output = EffectBoardCell;

    fn index(&self, sq: Sq) -> &Self::Output {
        &self.cells[sq.get() as usize]
    }
}

impl std::ops::IndexMut<Sq> for EffectBoard {
    fn index_mut(&mut self, sq: Sq) -> &mut Self::Output {
        &mut self.cells[sq.get() as usize]
    }
}
