//!===================================================================
//! my 側の指し手を扱うモジュール。
//!
//! 原作では my 側は打ち歩詰めを指さない。
//! また、成れる場合は必ず成る。
//!
//! 非定跡手については明示的な自殺手判定は行わない(指し手生成後の駒損判定で代用している)。
//!
//! 定跡手については明示的に合法手判定を行う。
//! このとき、玉が your 利きのあるマスに移動する手は弾く。
//!
//! my 側では指し手を以下のように分類する:
//!
//!   * pseudo-legal: 非定跡 my 合法手(自殺手含む)に打ち歩詰めを加えた集合
//!   * book-legal: 定跡 my 合法手
//!===================================================================

use boolinator::Boolinator;
use either::Either;

use crate::effect::{self, EffectBoard};
use crate::position::PawnMask;
use crate::prelude::*;

/// book-legal 判定。
///
/// src != dst などの条件は Move 生成時にチェック済み。
pub fn is_book_legal(pos: &Position, eff_board: &EffectBoard, mv: &Move) -> bool {
    match mv {
        Move::Nondrop(nondrop) => is_book_legal_nondrop(pos, eff_board, nondrop),
        Move::Drop(_) => unreachable!(), // 定跡手に drop は含まれないので、ここはサボる
    }
}

/// nondrop の book-legal 判定。
fn is_book_legal_nondrop(pos: &Position, eff_board: &EffectBoard, nondrop: &MoveNondrop) -> bool {
    let my = pos.side();
    let src = nondrop.src;
    let dst = nondrop.dst;
    let is_promotion = nondrop.is_promotion;

    if pos.board()[dst].is_side(my) {
        return false;
    }

    // src には自駒がなければならない
    let pt = pos.board()[src].piece_of(my);
    if pt.is_none() {
        return false;
    }
    let pt = pt.unwrap();

    let mut pt_dst = pt;
    if is_promotion {
        if !can_promote(my, pt, src, dst) {
            return false;
        }
        pt_dst = pt_dst.to_promoted().unwrap();
    }

    // 行きどころのない駒はNG
    if !dst.can_put(my, pt_dst) {
        return false;
    }

    // 駒種ごとに下請け関数で判定
    match pt {
        Piece::King => is_book_legal_nondrop_king(pos, eff_board, nondrop),
        Piece::Lance => is_book_legal_nondrop_lance(pos, nondrop),
        Piece::Bishop => is_book_legal_nondrop_bishop(pos, nondrop),
        Piece::Rook => is_book_legal_nondrop_rook(pos, nondrop),
        Piece::Horse => is_book_legal_nondrop_horse(pos, nondrop),
        Piece::Dragon => is_book_legal_nondrop_dragon(pos, nondrop),
        _ => is_book_legal_nondrop_melee(pos, nondrop, pt),
    }
}

/// 近接駒を動かす手の book-legal 判定。
fn is_book_legal_nondrop_melee(pos: &Position, nondrop: &MoveNondrop, pt: Piece) -> bool {
    let my = pos.side();
    let src = nondrop.src;
    let dst = nondrop.dst;

    pt.effects_melee(my).any(|di| di == dst.get() - src.get())
}

/// 玉を動かす手の book-legal 判定。
fn is_book_legal_nondrop_king(
    pos: &Position,
    eff_board: &EffectBoard,
    nondrop: &MoveNondrop,
) -> bool {
    let my = pos.side();
    let your = my.inv();
    let dst = nondrop.dst;

    // 玉を your 利きのあるマスに移動する手はNG
    if eff_board[dst][your].count() > 0 {
        return false;
    }

    is_book_legal_nondrop_melee(pos, nondrop, Piece::King)
}

/// 香を動かす手の book-legal 判定。
fn is_book_legal_nondrop_lance(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let my = pos.side();
    // 手番側から見たマスに直す
    let src = nondrop.src.rel(my);
    let dst = nondrop.dst.rel(my);

    // 横に動くのは違法
    if src.x() != dst.x() {
        return false;
    }

    // 後ろに動くのは違法
    if dst.y() > src.y() {
        return false;
    }

    let step = Sq::dist_y(src, dst).unwrap();
    is_book_legal_nondrop_ranged_dir(pos, nondrop, 11, step)
}

/// 角を動かす手の book-legal 判定。
fn is_book_legal_nondrop_bishop(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let src = nondrop.src;
    let dst = nondrop.dst;

    let dx = dst.x().get() - src.x().get();
    let dy = dst.y().get() - src.y().get();
    let step = dx.abs();

    if dx == dy {
        is_book_legal_nondrop_ranged_dir(pos, nondrop, 12, step)
    } else if dx == -dy {
        is_book_legal_nondrop_ranged_dir(pos, nondrop, 10, step)
    } else {
        false
    }
}

/// 飛を動かす手の book-legal 判定。
fn is_book_legal_nondrop_rook(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let src = nondrop.src;
    let dst = nondrop.dst;

    if src.y() == dst.y() {
        // 横移動
        let step = Sq::dist_x(src, dst).unwrap();
        is_book_legal_nondrop_ranged_dir(pos, nondrop, 1, step)
    } else {
        // 斜めに動くのは違法
        if src.x() != dst.x() {
            return false;
        }
        // 縦移動
        let step = Sq::dist_y(src, dst).unwrap();
        is_book_legal_nondrop_ranged_dir(pos, nondrop, 11, step)
    }
}

/// 馬を動かす手の book-legal 判定。
fn is_book_legal_nondrop_horse(pos: &Position, nondrop: &MoveNondrop) -> bool {
    // 距離 1 の移動は常に合法
    if Sq::dist(nondrop.src, nondrop.dst).unwrap() < 2 {
        return true;
    }

    is_book_legal_nondrop_bishop(pos, nondrop)
}

/// 龍を動かす手の book-legal 判定。
fn is_book_legal_nondrop_dragon(pos: &Position, nondrop: &MoveNondrop) -> bool {
    // 距離 1 の移動は常に合法
    if Sq::dist(nondrop.src, nondrop.dst).unwrap() < 2 {
        return true;
    }

    is_book_legal_nondrop_rook(pos, nondrop)
}

/// 遠隔移動の book-legal 判定。
///
/// min(nondrop.src, nondrop.dst) から dir 方向へ step 回動けるかどうかで判定する。
fn is_book_legal_nondrop_ranged_dir(
    pos: &Position,
    nondrop: &MoveNondrop,
    dir: i32,
    step: i32,
) -> bool {
    let src = std::cmp::min(nondrop.src, nondrop.dst);

    // 1 回は必ず動ける (dst が自駒でないことは確認済み)
    if step == 1 {
        return true;
    }
    let mut sq = src + dir;

    // step-1 回動かす。動かす前のマスが空白でなければ違法。
    for _ in 0..step - 1 {
        if !pos.board()[sq].is_empty() {
            return false;
        }
        sq += dir;
    }

    true
}

/// my 側の pseudo-legal 列挙。
/// 打ち歩詰め及び自殺手が含まれる。
pub fn moves_pseudo_legal(pos: &Position) -> impl Iterator<Item = Move> + '_ {
    let my = pos.side();

    Sq::iter_valid_sim(my).flat_map(move |sq| {
        if let Some(pt) = pos.board()[sq].piece_of(my) {
            Either::Left(moves_pseudo_legal_nondrop(pos, sq, pt))
        } else {
            Either::Right(moves_pseudo_legal_drop(pos, sq))
        }
    })
}

fn moves_pseudo_legal_nondrop(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    match pt {
        Piece::Lance => Either::Left(moves_pseudo_legal_nondrop_lance(pos, src)),
        Piece::Bishop => Either::Right(Either::Left(moves_pseudo_legal_nondrop_bishop(
            pos,
            src,
            Piece::Bishop,
        ))),
        Piece::Rook => Either::Right(Either::Right(Either::Left(
            moves_pseudo_legal_nondrop_rook(pos, src, Piece::Rook),
        ))),
        Piece::Horse => Either::Right(Either::Right(Either::Right(Either::Left(
            moves_pseudo_legal_nondrop_horse(pos, src),
        )))),
        Piece::Dragon => Either::Right(Either::Right(Either::Right(Either::Right(Either::Left(
            moves_pseudo_legal_nondrop_dragon(pos, src),
        ))))),
        _ => Either::Right(Either::Right(Either::Right(Either::Right(Either::Right(
            moves_pseudo_legal_nondrop_melee(pos, src, pt),
        ))))),
    }
}

fn moves_pseudo_legal_nondrop_melee(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    let my = pos.side();
    let your = my.inv();

    effect::piece_effects_melee(my, pt).filter_map(move |di| {
        let dst = src + di;
        // 成れるなら必ず成る
        let is_promotion = can_promote(my, pt, src, dst);
        let cell = pos.board()[dst];
        (cell.is_empty() || cell.is_side(your))
            .as_some_from(|| Move::nondrop(src, dst, is_promotion))
    })
}

fn moves_pseudo_legal_nondrop_ranged(
    pos: &Position,
    src: Sq,
    pt: Piece,
    dir: i32,
) -> impl Iterator<Item = Move> + '_ {
    let my = pos.side();
    let your = my.inv();

    let mut dst = src + dir;
    let it = move || {
        let nxt = dst + dir;
        std::mem::replace(&mut dst, nxt)
    };

    std::iter::repeat_with(it).scan(true, move |ok, dst| {
        if !*ok {
            return None;
        }
        // your 駒があれば次で打ち切り
        if pos.board()[dst].is_side(your) {
            *ok = false;
        }
        // my 駒または壁ならば即打ち切り
        else if !pos.board()[dst].is_empty() {
            return None;
        }
        // 成れるなら必ず成る
        let is_promotion = can_promote(my, pt, src, dst);
        Some(Move::nondrop(src, dst, is_promotion))
    })
}

fn moves_pseudo_legal_nondrop_lance(pos: &Position, src: Sq) -> impl Iterator<Item = Move> + '_ {
    let my = pos.side();
    moves_pseudo_legal_nondrop_ranged(pos, src, Piece::Lance, -11 * my.sgn())
}

fn moves_pseudo_legal_nondrop_bishop(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    const DIRS: &[i32] = &[12, 10, -10, -12];
    let my = pos.side();
    DIRS.iter()
        .flat_map(move |&dir| moves_pseudo_legal_nondrop_ranged(pos, src, pt, dir * my.sgn()))
}

fn moves_pseudo_legal_nondrop_rook(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    const DIRS: &[i32] = &[11, -11, 1, -1];
    let my = pos.side();
    DIRS.iter()
        .flat_map(move |&dir| moves_pseudo_legal_nondrop_ranged(pos, src, pt, dir * my.sgn()))
}

fn moves_pseudo_legal_nondrop_horse(pos: &Position, src: Sq) -> impl Iterator<Item = Move> + '_ {
    itertools::chain(
        moves_pseudo_legal_nondrop_bishop(pos, src, Piece::Horse),
        moves_pseudo_legal_nondrop_melee(pos, src, Piece::Horse),
    )
}

fn moves_pseudo_legal_nondrop_dragon(pos: &Position, src: Sq) -> impl Iterator<Item = Move> + '_ {
    itertools::chain(
        moves_pseudo_legal_nondrop_rook(pos, src, Piece::Dragon),
        moves_pseudo_legal_nondrop_melee(pos, src, Piece::Dragon),
    )
}

fn moves_pseudo_legal_drop(pos: &Position, dst: Sq) -> impl Iterator<Item = Move> + '_ {
    // 原作の指し手生成順に合わせる
    const PTS: &[Piece] = &[
        Piece::Pawn,
        Piece::Lance,
        Piece::Knight,
        Piece::Silver,
        Piece::Gold,
        Piece::Bishop,
        Piece::Rook,
    ];

    let my = pos.side();
    let pawn_mask = PawnMask::from_board_side(pos.board(), my);

    let is_ok = move |drop: &MoveDrop| -> bool {
        let pt = drop.pt;
        let dst = drop.dst;

        // 移動先が空白でないならNG
        if !pos.board()[dst].is_empty() {
            return false;
        }

        // 行きどころのない駒はNG
        if !dst.can_put(my, pt) {
            return false;
        }

        // 二歩はNG
        if matches!(pt, Piece::Pawn) && pawn_mask.test(dst.x().get()) {
            return false;
        }

        true
    };

    let pts = PTS.iter().filter(move |&&pt| pos.hand(my)[pt] > 0);

    pts.map(move |&pt| MoveDrop::new(pt, dst))
        .filter(is_ok)
        .map(Move::Drop)
}
