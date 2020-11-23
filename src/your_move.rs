//!===================================================================
//! your 側の指し手を扱うモジュール。
//!
//! 原作をそこそこ忠実に移植。
//! 原作では your 側の打ち歩詰めは許される。また、your 側は自殺手を指して負けることができる。
//!
//! your 側では指し手を以下のように分類する:
//!
//!   * pseudo-legal: 原作でプレイヤーが実際に指せる手の集合
//!   * legal: pseudo-legal から自殺手を除いた集合
//!===================================================================

use boolinator::Boolinator;
use either::Either;

use crate::ai;
use crate::effect;
use crate::position::PawnMask;
use crate::prelude::*;

/// your 側の指し手の疑似合法性判定。
/// 打ち歩詰め及び自殺手は許される。
/// テスト用。思考ルーチンでは使われない。
///
/// src != dst などの条件は Move 生成時にチェック済み。
pub fn is_pseudo_legal(pos: &Position, mv: &Move) -> bool {
    match mv {
        Move::Nondrop(nondrop) => is_pseudo_legal_nondrop(pos, nondrop),
        Move::Drop(drop) => is_pseudo_legal_drop(pos, drop),
    }
}

/// nondrop の疑似合法性判定。
fn is_pseudo_legal_nondrop(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let your = pos.side();
    let src = nondrop.src;
    let dst = nondrop.dst;
    let is_promotion = nondrop.is_promotion;

    if pos.board()[dst].is_side(your) {
        return false;
    }

    // src には自駒がなければならない
    let pt = pos.board()[src].piece_of(your);
    if pt.is_none() {
        return false;
    }
    let pt = pt.unwrap();

    let mut pt_dst = pt;
    if is_promotion {
        // 違法な成りを弾く
        if !src.can_promote(your) && !dst.can_promote(your) {
            return false;
        }
        if !pt.can_promote() {
            return false;
        }
        pt_dst = pt_dst.to_promoted().unwrap();
    }

    // 行きどころのない駒はNG
    if !dst.can_put(your, pt_dst) {
        return false;
    }

    // 駒種ごとに下請け関数で判定
    match pt {
        Piece::Lance => is_pseudo_legal_nondrop_lance(pos, nondrop),
        Piece::Bishop => is_pseudo_legal_nondrop_bishop(pos, nondrop),
        Piece::Rook => is_pseudo_legal_nondrop_rook(pos, nondrop),
        Piece::Horse => is_pseudo_legal_nondrop_horse(pos, nondrop),
        Piece::Dragon => is_pseudo_legal_nondrop_dragon(pos, nondrop),
        _ => is_pseudo_legal_nondrop_melee(pos, nondrop, pt),
    }
}

/// 近接駒を動かす手の疑似合法性判定。
fn is_pseudo_legal_nondrop_melee(pos: &Position, nondrop: &MoveNondrop, pt: Piece) -> bool {
    let your = pos.side();
    let src = nondrop.src;
    let dst = nondrop.dst;

    pt.effects_melee(your).any(|di| di == dst.get() - src.get())
}

/// 香を動かす手の疑似合法性判定。
fn is_pseudo_legal_nondrop_lance(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let your = pos.side();
    // 手番側から見たマスに直す
    let src = nondrop.src.rel(your);
    let dst = nondrop.dst.rel(your);

    // 横に動くのは違法
    if src.x() != dst.x() {
        return false;
    }

    // 後ろに動くのは違法
    if dst.y() > src.y() {
        return false;
    }

    let step = Sq::dist_y(src, dst).unwrap();
    is_pseudo_legal_nondrop_ranged_dir(pos, nondrop, 11, step)
}

/// 角を動かす手の疑似合法性判定。
fn is_pseudo_legal_nondrop_bishop(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let src = nondrop.src;
    let dst = nondrop.dst;

    let dx = dst.x().get() - src.x().get();
    let dy = dst.y().get() - src.y().get();
    let step = dx.abs();

    if dx == dy {
        is_pseudo_legal_nondrop_ranged_dir(pos, nondrop, 12, step)
    } else if dx == -dy {
        is_pseudo_legal_nondrop_ranged_dir(pos, nondrop, 10, step)
    } else {
        false
    }
}

/// 飛を動かす手の疑似合法性判定。
fn is_pseudo_legal_nondrop_rook(pos: &Position, nondrop: &MoveNondrop) -> bool {
    let src = nondrop.src;
    let dst = nondrop.dst;

    if src.y() == dst.y() {
        // 横移動
        let step = Sq::dist_x(src, dst).unwrap();
        is_pseudo_legal_nondrop_ranged_dir(pos, nondrop, 1, step)
    } else {
        // 斜めに動くのは違法
        if src.x() != dst.x() {
            return false;
        }
        // 縦移動
        let step = Sq::dist_y(src, dst).unwrap();
        is_pseudo_legal_nondrop_ranged_dir(pos, nondrop, 11, step)
    }
}

/// 馬を動かす手の疑似合法性判定。
fn is_pseudo_legal_nondrop_horse(pos: &Position, nondrop: &MoveNondrop) -> bool {
    // 距離 1 の移動は常に合法
    if Sq::dist(nondrop.src, nondrop.dst).unwrap() < 2 {
        return true;
    }

    is_pseudo_legal_nondrop_bishop(pos, nondrop)
}

/// 龍を動かす手の疑似合法性判定。
fn is_pseudo_legal_nondrop_dragon(pos: &Position, nondrop: &MoveNondrop) -> bool {
    // 距離 1 の移動は常に合法
    if Sq::dist(nondrop.src, nondrop.dst).unwrap() < 2 {
        return true;
    }

    is_pseudo_legal_nondrop_rook(pos, nondrop)
}

/// 遠隔移動の疑似合法性判定
///
/// min(nondrop.src, nondrop.dst) から dir 方向へ step 回動けるかどうかで判定する。
fn is_pseudo_legal_nondrop_ranged_dir(
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

/// drop の疑似合法性判定。
fn is_pseudo_legal_drop(pos: &Position, drop: &MoveDrop) -> bool {
    let your = pos.side();
    let pt = drop.pt;
    let dst = drop.dst;

    if !pos.board()[dst].is_empty() {
        return false;
    }

    // 持ってない駒を打つのはNG
    if pos.hand(your)[pt] == 0 {
        return false;
    }

    // 行きどころのない駒はNG
    if !dst.can_put(your, pt) {
        return false;
    }

    // 二歩チェック
    if matches!(pt, Piece::Pawn) {
        let pawn_mask = PawnMask::from_board_side(pos.board(), your);
        if pawn_mask.test(dst.x().get()) {
            return false;
        }
    }

    true
}

/// your 側の王手回避手を列挙する。
/// 成れる場合は必ず成る(原作での暗黙の仮定)。
/// 実際には王手を回避しない手も含まれるので、呼び出し側で適宜調べること。
/// 順序は原作と異なるが、思考ルーチンの動作に影響はないはず。
pub fn moves_evasion(pos: &Position) -> impl Iterator<Item = Move> + '_ {
    let your = pos.side();

    let sq_king_your = ai::find_king_sq(pos.board(), your).unwrap();

    let it_nondrop_king = moves_evasion_nondrop(pos, sq_king_your, Piece::King);

    let it_drop = moves_evasion_drop(pos, sq_king_your);

    let sqs = Sq::iter_valid().filter(move |&sq| sq != sq_king_your);
    let it_nondrop_nonking = sqs
        .filter_map(move |src| {
            pos.board()[src]
                .piece_of(your)
                .map(|pt| moves_evasion_nondrop(pos, src, pt))
        })
        .flatten();

    it_nondrop_king.chain(it_drop).chain(it_nondrop_nonking)
}

/// your 側の (src, pt) による nondrop 王手回避手を列挙する。
fn moves_evasion_nondrop(pos: &Position, src: Sq, pt: Piece) -> impl Iterator<Item = Move> + '_ {
    match pt {
        Piece::Lance => Either::Left(moves_evasion_nondrop_lance(pos, src)),
        Piece::Bishop => Either::Right(Either::Left(moves_evasion_nondrop_bishop(
            pos,
            src,
            Piece::Bishop,
        ))),
        Piece::Rook => Either::Right(Either::Right(Either::Left(moves_evasion_nondrop_rook(
            pos,
            src,
            Piece::Rook,
        )))),
        Piece::Horse => Either::Right(Either::Right(Either::Right(Either::Left(
            moves_evasion_nondrop_horse(pos, src),
        )))),
        Piece::Dragon => Either::Right(Either::Right(Either::Right(Either::Right(Either::Left(
            moves_evasion_nondrop_dragon(pos, src),
        ))))),
        _ => Either::Right(Either::Right(Either::Right(Either::Right(Either::Right(
            moves_evasion_nondrop_melee(pos, src, pt),
        ))))),
    }
}

fn moves_evasion_nondrop_melee(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    let your = pos.side();
    let my = your.inv();

    effect::piece_effects_melee(your, pt).filter_map(move |di| {
        let dst = src + di;
        // 成れるなら必ず成る
        let is_promotion = can_promote(your, pt, src, dst);
        let cell = pos.board()[dst];
        (cell.is_empty() || cell.is_side(my)).as_some_from(|| Move::nondrop(src, dst, is_promotion))
    })
}

fn moves_evasion_nondrop_ranged(
    pos: &Position,
    src: Sq,
    pt: Piece,
    dir: i32,
) -> impl Iterator<Item = Move> + '_ {
    let your = pos.side();
    let my = your.inv();

    let mut dst = src + dir;
    let it = move || {
        let nxt = dst + dir;
        std::mem::replace(&mut dst, nxt)
    };

    std::iter::repeat_with(it).scan(true, move |ok, dst| {
        if !*ok {
            return None;
        }
        // my 駒があれば次で打ち切り
        if pos.board()[dst].is_side(my) {
            *ok = false;
        }
        // your 駒または壁ならば即打ち切り
        else if !pos.board()[dst].is_empty() {
            return None;
        }
        // 成れるなら必ず成る
        let is_promotion = can_promote(your, pt, src, dst);
        Some(Move::nondrop(src, dst, is_promotion))
    })
}

fn moves_evasion_nondrop_lance(pos: &Position, src: Sq) -> impl Iterator<Item = Move> + '_ {
    let your = pos.side();
    moves_evasion_nondrop_ranged(pos, src, Piece::Lance, -11 * your.sgn())
}

fn moves_evasion_nondrop_bishop(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    const DIRS: &[i32] = &[12, 10, -10, -12];
    let your = pos.side();
    DIRS.iter()
        .flat_map(move |&dir| moves_evasion_nondrop_ranged(pos, src, pt, dir * your.sgn()))
}

fn moves_evasion_nondrop_rook(
    pos: &Position,
    src: Sq,
    pt: Piece,
) -> impl Iterator<Item = Move> + '_ {
    const DIRS: &[i32] = &[11, -11, 1, -1];
    let your = pos.side();
    DIRS.iter()
        .flat_map(move |&dir| moves_evasion_nondrop_ranged(pos, src, pt, dir * your.sgn()))
}

fn moves_evasion_nondrop_horse(pos: &Position, src: Sq) -> impl Iterator<Item = Move> + '_ {
    itertools::chain(
        moves_evasion_nondrop_bishop(pos, src, Piece::Horse),
        moves_evasion_nondrop_melee(pos, src, Piece::Horse),
    )
}

fn moves_evasion_nondrop_dragon(pos: &Position, src: Sq) -> impl Iterator<Item = Move> + '_ {
    itertools::chain(
        moves_evasion_nondrop_rook(pos, src, Piece::Dragon),
        moves_evasion_nondrop_melee(pos, src, Piece::Dragon),
    )
}

/// your 側の drop 王手回避手を列挙する。
/// 玉周り最大 9 マスしか調べないので、入玉形だと詰み判定を誤るケースがありうる。
fn moves_evasion_drop(pos: &Position, sq_king_your: Sq) -> impl Iterator<Item = Move> + '_ {
    const NEIGHBOR: &[i32] = &[-12, -11, -10, -1, 0, 1, 10, 11, 12];

    let your = pos.side();
    let pawn_mask = PawnMask::from_board_side(pos.board(), your);

    let is_ok = move |drop: &MoveDrop| -> bool {
        let pt = drop.pt;
        let dst = drop.dst;

        // 移動先が空白でないならNG
        if !pos.board()[dst].is_empty() {
            return false;
        }

        // 行きどころのない駒はNG
        if !dst.can_put(your, pt) {
            return false;
        }

        // 二歩はNG
        if matches!(pt, Piece::Pawn) && pawn_mask.test(dst.x().get()) {
            return false;
        }

        true
    };

    let sqs = NEIGHBOR.iter().filter_map(move |di| {
        let sq = sq_king_your + *di;
        sq.is_valid().as_some(sq)
    });

    sqs.flat_map(move |dst| {
        let pts = Piece::iter_hand().filter(move |&pt| pos.hand(your)[pt] > 0);
        pts.map(move |pt| MoveDrop::new(pt, dst))
    })
    .filter(is_ok)
    .map(Move::Drop)
}

/// your 側の合法手を列挙する。
/// 打ち歩詰めは含まれるが、自殺手は含まれない。
/// テスト用。思考ルーチンでは使われない。
pub fn moves_legal(pos: &mut Position) -> impl Iterator<Item = Move> {
    let mut mvs: Vec<_> = moves_pseudo_legal(pos).collect();

    mvs.retain(|mv| {
        let cmd = pos.do_move(mv).unwrap();
        let ok = !pos.can_capture_king();
        pos.undo_move(&cmd).unwrap();
        ok
    });

    mvs.into_iter()
}

/// your 側の疑似合法手を列挙する。
/// これは原作で your 側が指せる手の集合と一致する。
/// 打ち歩詰めと自殺手が含まれる。
/// テスト用。思考ルーチンでは使われない。
pub fn moves_pseudo_legal(pos: &Position) -> impl Iterator<Item = Move> + '_ {
    itertools::chain(
        moves_pseudo_legal_nondrop(pos),
        moves_pseudo_legal_drop(pos),
    )
}

fn moves_pseudo_legal_nondrop(pos: &Position) -> impl Iterator<Item = Move> + '_ {
    let your = pos.side();

    let is_ok = move |nondrop: &MoveNondrop| -> bool {
        let src = nondrop.src;
        let dst = nondrop.dst;
        let is_promotion = nondrop.is_promotion;

        // 移動先が your 駒ならNG
        if pos.board()[dst].is_side(your) {
            return false;
        }

        // 成り処理
        let mut pt = pos.board()[src].piece_of(your).unwrap();
        if is_promotion {
            if !can_promote(your, pt, src, dst) {
                return false;
            }
            pt = pt.to_promoted().unwrap();
        }

        // 行きどころのない駒はNG
        if !dst.can_put(your, pt) {
            return false;
        }

        true
    };

    moves_illegal_nondrop(pos).filter(is_ok).map(Move::Nondrop)
}

fn moves_pseudo_legal_drop(pos: &Position) -> impl Iterator<Item = Move> + '_ {
    let your = pos.side();
    let pawn_mask = PawnMask::from_board_side(pos.board(), your);

    let is_ok = move |drop: &MoveDrop| -> bool {
        let pt = drop.pt;
        let dst = drop.dst;

        // 移動先が空白でないならNG
        if !pos.board()[dst].is_empty() {
            return false;
        }

        // 行きどころのない駒はNG
        if !dst.can_put(your, pt) {
            return false;
        }

        // 二歩はNG
        if matches!(pt, Piece::Pawn) && pawn_mask.test(dst.x().get()) {
            return false;
        }

        true
    };

    moves_illegal_drop(pos).filter(is_ok).map(Move::Drop)
}

/// 違法手も含む指し手を列挙する。
#[cfg(test)]
fn moves_illegal(pos: &Position) -> impl Iterator<Item = Move> + '_ {
    itertools::chain(
        moves_illegal_nondrop(pos).map(Move::Nondrop),
        moves_illegal_drop(pos).map(Move::Drop),
    )
}

/// 違法手も含む MoveNondrop を列挙する。
fn moves_illegal_nondrop(pos: &Position) -> impl Iterator<Item = MoveNondrop> + '_ {
    let your = pos.side();

    effect::iter_effects(pos.board(), your)
        .filter(|&(_, dst)| dst.is_valid())
        .flat_map(move |(src, dst)| {
            [false, true]
                .iter()
                .map(move |is_promotion| MoveNondrop::new(src, dst, *is_promotion))
        })
}

/// 違法手も含む Drop を列挙する。
fn moves_illegal_drop(pos: &Position) -> impl Iterator<Item = MoveDrop> + '_ {
    let your = pos.side();

    let pts = Piece::iter_hand().filter(move |&pt| pos.hand(your)[pt] > 0);

    pts.flat_map(|pt| Sq::iter_valid().map(move |dst| MoveDrop::new(pt, dst)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    #[test]
    fn test() {
        for _ in 0..100 {
            let pos = Position::random(&mut rand::thread_rng());
            let mvs_gen: HashSet<_> = moves_pseudo_legal(&pos).collect();

            // 生成した疑似合法手が全て判定を通るか?
            assert!(mvs_gen.iter().all(|mv| is_pseudo_legal(&pos, mv)));

            // 違法手をフィルタリングした集合と一致するか?
            let mvs_filt: HashSet<_> = moves_illegal(&pos)
                .filter(|mv| is_pseudo_legal(&pos, mv))
                .collect();
            assert_eq!(mvs_gen, mvs_filt);
        }
    }
}
