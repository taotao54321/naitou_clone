//!===================================================================
//! 定跡
//!
//! 定跡データは戦型別の「定跡分岐」と「定跡手順」から成る。
//!
//! 定跡分岐とは、「your 側のこの手にはこう応じるべし」という指示。
//! (正確には「このマスに your 側のこの駒があったら...」の意)
//! 具体的な応手指示と、戦型変更指示の2種がある。
//!
//! 定跡手順とは、「この戦型ではこの手順で指し進めるべし」という指示。
//!
//! 定跡データ内のマスは my 側を後手として記述されている。
//!===================================================================

use crate::prelude::*;

//--------------------------------------------------------------------
// util
//--------------------------------------------------------------------

const fn bit_test(x: u32, bit: usize) -> bool {
    (x & (1 << bit)) != 0
}

const fn bit_set(x: u32, bit: usize) -> u32 {
    x | (1 << bit)
}

const fn bit_clear(x: u32, bit: usize) -> u32 {
    x & !(1 << bit)
}

const fn bit_assign(x: u32, bit: usize, value: bool) -> u32 {
    if value {
        bit_set(x, bit)
    } else {
        bit_clear(x, bit)
    }
}

/// 戦型
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Formation {
    Nakabisha,
    Sikenbisha,
    Kakugawari,
    Sujichigai,
    YourHishaochi,
    YourNimaiochi,
    MyHishaochi,
    MyNimaiochi,
    Nothing,
}

impl Formation {
    pub fn from_handicap(handicap: Handicap, timelimit: bool) -> Self {
        match handicap {
            Handicap::YourSente | Handicap::MySente => {
                if timelimit {
                    Self::Nakabisha
                } else {
                    Self::Sikenbisha
                }
            }
            Handicap::YourHishaochi => Self::YourHishaochi,
            Handicap::YourNimaiochi => Self::YourNimaiochi,
            Handicap::MyHishaochi => Self::MyHishaochi,
            Handicap::MyNimaiochi => Self::MyNimaiochi,
        }
    }
}

/// 定跡分岐エントリ: 応手指示
/// sq_your に pt_your があったら (src_my, dst_my) で応じる
#[derive(Clone, Debug, Eq, PartialEq)]
struct BookBranchMove {
    sq_your: Sq,
    pt_your: Piece,
    src_my: Sq,
    dst_my: Sq,
}

/// 定跡分岐エントリ: 戦型変更指示
/// 手数 ply 以内で sq_your に pt_your があったら戦型を formation に変更
#[derive(Clone, Debug, Eq, PartialEq)]
struct BookBranchChange {
    sq_your: Sq,
    pt_your: Piece,
    formation: Formation,
    ply: u8,
}

/// 定跡分岐エントリ
#[derive(Clone, Debug, Eq, PartialEq)]
enum BookBranchEntry {
    Move(BookBranchMove),
    Change(BookBranchChange),
}

impl BookBranchEntry {
    const fn new_move(sq_your: Sq, pt_your: Piece, src_my: Sq, dst_my: Sq) -> Self {
        Self::Move(BookBranchMove {
            sq_your,
            pt_your,
            src_my,
            dst_my,
        })
    }

    const fn new_change(sq_your: Sq, pt_your: Piece, formation: Formation, ply: u8) -> Self {
        Self::Change(BookBranchChange {
            sq_your,
            pt_your,
            formation,
            ply,
        })
    }
}

/// 定跡手順エントリ
/// nondrop な指し手の src, dst。必ず不成になる。
#[derive(Clone, Debug, Eq, PartialEq)]
struct BookMovesEntry {
    src_my: Sq,
    dst_my: Sq,
}

impl BookMovesEntry {
    const fn new(src_my: Sq, dst_my: Sq) -> Self {
        Self { src_my, dst_my }
    }
}

/// 定跡データは my 側を後手としているので、手番による補正を行う。
fn book_sq(sq: Sq, my: Side) -> Sq {
    match my {
        Side::Sente => sq.inv(),
        Side::Gote => sq,
    }
}

/// 定跡処理用状態データ
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookState {
    pub formation: Formation,
    pub done_branch: u32, // 定跡分岐エントリ処理済みフラグ (size: 16)
    pub done_moves: u32,  // 定跡手順エントリ処理済みフラグ (size: 24)
}

impl BookState {
    pub fn new(formation: Formation) -> Self {
        Self {
            formation,
            done_branch: 0,
            done_moves: 0,
        }
    }

    pub fn formation(&self) -> Formation {
        self.formation
    }

    fn change_formation(&mut self, formation: Formation) {
        self.formation = formation;
        self.done_branch = 0;
        self.done_moves = 0;
    }

    /// 定跡手を得る。
    /// 戦型が Formation::Nothing になった場合、None を返す。
    ///
    /// my 先手の場合、初手のみ定跡手を指しても done フラグが立たない(原作通り)。
    ///
    /// 合法性チェックや駒損チェックは行わないので、呼び出し側で適切に処理すること。
    pub fn process(&mut self, pos: &Position, progress_ply: u8) -> Option<Move> {
        assert_ne!(self.formation, Formation::Nothing);

        let my = pos.side();
        let your = my.inv();

        // 定跡分岐
        'outer: loop {
            for (i, e) in get_book_branch(self.formation).iter().enumerate() {
                if bit_test(self.done_branch, i) {
                    continue;
                }
                match e {
                    BookBranchEntry::Move(bra_mv) => {
                        let sq_your = book_sq(bra_mv.sq_your, my);
                        let pt_your = bra_mv.pt_your;
                        let src_my = book_sq(bra_mv.src_my, my);
                        let dst_my = book_sq(bra_mv.dst_my, my);
                        if pos.board()[sq_your].is_side_pt(your, pt_your) {
                            self.done_branch = bit_assign(self.done_branch, i, progress_ply != 0);
                            return Some(Move::nondrop(src_my, dst_my, false));
                        }
                    }
                    BookBranchEntry::Change(bra_ch) => {
                        let sq_your = book_sq(bra_ch.sq_your, my);
                        let pt_your = bra_ch.pt_your;
                        let formation = bra_ch.formation;
                        let ply = bra_ch.ply;
                        if pos.board()[sq_your].is_side_pt(your, pt_your) && progress_ply <= ply {
                            // 戦型変更したら定跡分岐探索からやり直し
                            self.change_formation(formation);
                            continue 'outer;
                        }
                    }
                }
            }
            break;
        }

        // 定跡手順
        for (i, e) in get_book_moves(self.formation).iter().enumerate() {
            if bit_test(self.done_moves, i) {
                continue;
            }
            self.done_moves = bit_assign(self.done_moves, i, progress_ply != 0);
            let src_my = book_sq(e.src_my, my);
            let dst_my = book_sq(e.dst_my, my);
            return Some(Move::nondrop(src_my, dst_my, false));
        }

        self.formation = Formation::Nothing;
        None
    }
}

fn get_book_branch(formation: Formation) -> &'static [BookBranchEntry] {
    match formation {
        Formation::Nakabisha => BRANCH_NAKABISHA,
        Formation::Sikenbisha => BRANCH_SIKENBISHA,
        Formation::Kakugawari => BRANCH_KAKUGAWARI,
        Formation::Sujichigai => BRANCH_SUJICHIGAI,
        Formation::YourHishaochi => BRANCH_YOUR_HISHAOCHI,
        Formation::YourNimaiochi => BRANCH_YOUR_NIMAIOCHI,
        Formation::MyHishaochi => BRANCH_MY_HISHAOCHI,
        Formation::MyNimaiochi => BRANCH_MY_NIMAIOCHI,
        _ => unreachable!(),
    }
}

fn get_book_moves(formation: Formation) -> &'static [BookMovesEntry] {
    match formation {
        Formation::Nakabisha => MOVES_NAKABISHA,
        Formation::Sikenbisha => MOVES_SIKENBISHA,
        Formation::Kakugawari => MOVES_KAKUGAWARI,
        Formation::Sujichigai => MOVES_SUJICHIGAI,
        Formation::YourHishaochi => MOVES_YOUR_HISHAOCHI,
        Formation::YourNimaiochi => MOVES_YOUR_NIMAIOCHI,
        Formation::MyHishaochi => MOVES_MY_HISHAOCHI,
        Formation::MyNimaiochi => MOVES_MY_NIMAIOCHI,
        _ => unreachable!(),
    }
}

const BRANCH_NAKABISHA: &[BookBranchEntry] = &[
    BookBranchEntry::new_change(Sq::from_xy(8, 2), Piece::Bishop, Formation::Kakugawari, 5),
    BookBranchEntry::new_change(Sq::from_xy(8, 2), Piece::Horse, Formation::Kakugawari, 5),
    BookBranchEntry::new_move(
        Sq::from_xy(5, 5),
        Piece::Bishop,
        Sq::from_xy(5, 3),
        Sq::from_xy(5, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(6, 6),
        Piece::Bishop,
        Sq::from_xy(6, 4),
        Sq::from_xy(6, 5),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(6, 6),
        Piece::Silver,
        Sq::from_xy(6, 4),
        Sq::from_xy(6, 5),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(8, 6),
        Piece::Silver,
        Sq::from_xy(6, 1),
        Sq::from_xy(7, 2),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(6, 6),
        Piece::Pawn,
        Sq::from_xy(8, 2),
        Sq::from_xy(7, 3),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(8, 5),
        Piece::Pawn,
        Sq::from_xy(8, 2),
        Sq::from_xy(7, 3),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(7, 5),
        Piece::Silver,
        Sq::from_xy(6, 4),
        Sq::from_xy(6, 5),
    ),
];

const BRANCH_SIKENBISHA: &[BookBranchEntry] = &[
    BookBranchEntry::new_change(Sq::from_xy(8, 2), Piece::Bishop, Formation::Kakugawari, 5),
    BookBranchEntry::new_change(Sq::from_xy(8, 2), Piece::Horse, Formation::Kakugawari, 5),
    BookBranchEntry::new_move(
        Sq::from_xy(5, 5),
        Piece::Bishop,
        Sq::from_xy(5, 3),
        Sq::from_xy(5, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(6, 6),
        Piece::Bishop,
        Sq::from_xy(6, 4),
        Sq::from_xy(6, 5),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(6, 6),
        Piece::Silver,
        Sq::from_xy(6, 4),
        Sq::from_xy(6, 5),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(8, 6),
        Piece::Silver,
        Sq::from_xy(6, 2),
        Sq::from_xy(7, 2),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(6, 6),
        Piece::Pawn,
        Sq::from_xy(8, 2),
        Sq::from_xy(7, 3),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(8, 5),
        Piece::Pawn,
        Sq::from_xy(8, 2),
        Sq::from_xy(7, 3),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(7, 5),
        Piece::Silver,
        Sq::from_xy(6, 4),
        Sq::from_xy(6, 5),
    ),
];

const BRANCH_KAKUGAWARI: &[BookBranchEntry] = &[
    BookBranchEntry::new_change(Sq::from_xy(6, 5), Piece::Bishop, Formation::Sujichigai, 5),
    BookBranchEntry::new_change(Sq::from_xy(5, 6), Piece::Bishop, Formation::Sujichigai, 5),
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
];

const BRANCH_SUJICHIGAI: &[BookBranchEntry] = &[
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(9, 6),
        Piece::Pawn,
        Sq::from_xy(9, 3),
        Sq::from_xy(9, 4),
    ),
];

const BRANCH_YOUR_HISHAOCHI: &[BookBranchEntry] = &[
    BookBranchEntry::new_move(
        Sq::from_xy(9, 6),
        Piece::Pawn,
        Sq::from_xy(9, 3),
        Sq::from_xy(9, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(8, 2),
        Piece::Bishop,
        Sq::from_xy(7, 1),
        Sq::from_xy(8, 2),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(8, 2),
        Piece::Horse,
        Sq::from_xy(7, 1),
        Sq::from_xy(8, 2),
    ),
];

const BRANCH_YOUR_NIMAIOCHI: &[BookBranchEntry] = &[BookBranchEntry::new_move(
    Sq::from_xy(5, 6),
    Piece::Pawn,
    Sq::from_xy(5, 3),
    Sq::from_xy(5, 4),
)];

const BRANCH_MY_HISHAOCHI: &[BookBranchEntry] = &[
    BookBranchEntry::new_move(
        Sq::from_xy(8, 5),
        Piece::Pawn,
        Sq::from_xy(8, 2),
        Sq::from_xy(7, 3),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(9, 6),
        Piece::Pawn,
        Sq::from_xy(9, 3),
        Sq::from_xy(9, 4),
    ),
];

const BRANCH_MY_NIMAIOCHI: &[BookBranchEntry] = &[
    BookBranchEntry::new_move(
        Sq::from_xy(9, 6),
        Piece::Pawn,
        Sq::from_xy(9, 3),
        Sq::from_xy(9, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(1, 6),
        Piece::Pawn,
        Sq::from_xy(1, 3),
        Sq::from_xy(1, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(5, 6),
        Piece::Pawn,
        Sq::from_xy(5, 3),
        Sq::from_xy(5, 4),
    ),
    BookBranchEntry::new_move(
        Sq::from_xy(7, 5),
        Piece::Pawn,
        Sq::from_xy(7, 1),
        Sq::from_xy(8, 2),
    ),
];

const MOVES_NAKABISHA: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 3), Sq::from_xy(6, 4)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(2, 2), Sq::from_xy(5, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 2), Sq::from_xy(6, 3)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(3, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(8, 2), Sq::from_xy(7, 3)),
    BookMovesEntry::new(Sq::from_xy(5, 3), Sq::from_xy(5, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 2), Sq::from_xy(5, 3)),
    BookMovesEntry::new(Sq::from_xy(5, 2), Sq::from_xy(8, 2)),
    BookMovesEntry::new(Sq::from_xy(8, 3), Sq::from_xy(8, 4)),
    BookMovesEntry::new(Sq::from_xy(8, 4), Sq::from_xy(8, 5)),
    BookMovesEntry::new(Sq::from_xy(6, 4), Sq::from_xy(6, 5)),
];

const MOVES_SIKENBISHA: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 3), Sq::from_xy(6, 4)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(2, 2), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(7, 2), Sq::from_xy(6, 3)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(3, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 2), Sq::from_xy(2, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(3, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(5, 2)),
    BookMovesEntry::new(Sq::from_xy(8, 2), Sq::from_xy(7, 3)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(5, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 2), Sq::from_xy(6, 1)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 4), Sq::from_xy(6, 5)),
];

const MOVES_KAKUGAWARI: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(8, 2)),
    BookMovesEntry::new(Sq::from_xy(8, 2), Sq::from_xy(7, 3)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(2, 3), Sq::from_xy(2, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(2, 4), Sq::from_xy(2, 5)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(5, 2)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(6, 1)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 1)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(8, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 3), Sq::from_xy(6, 4)),
    BookMovesEntry::new(Sq::from_xy(5, 2), Sq::from_xy(6, 3)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 1), Sq::from_xy(3, 3)),
    BookMovesEntry::new(Sq::from_xy(4, 4), Sq::from_xy(4, 5)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(5, 4)),
];

const MOVES_SUJICHIGAI: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(8, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(5, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(8, 2), Sq::from_xy(7, 3)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(2, 3), Sq::from_xy(2, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 4), Sq::from_xy(2, 5)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(6, 1)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(5, 3), Sq::from_xy(5, 4)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 1), Sq::from_xy(3, 3)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(9, 3), Sq::from_xy(9, 4)),
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(6, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 4), Sq::from_xy(4, 5)),
];

const MOVES_YOUR_HISHAOCHI: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 3), Sq::from_xy(2, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 4), Sq::from_xy(2, 5)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(5, 2)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(6, 1)),
    BookMovesEntry::new(Sq::from_xy(5, 3), Sq::from_xy(5, 4)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(2, 1), Sq::from_xy(3, 3)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(9, 3), Sq::from_xy(9, 4)),
    BookMovesEntry::new(Sq::from_xy(8, 2), Sq::from_xy(7, 3)),
    BookMovesEntry::new(Sq::from_xy(4, 4), Sq::from_xy(4, 5)),
];

const MOVES_YOUR_NIMAIOCHI: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 4), Sq::from_xy(4, 5)),
    BookMovesEntry::new(Sq::from_xy(2, 2), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(3, 4), Sq::from_xy(3, 5)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(3, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 2), Sq::from_xy(3, 3)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(5, 2)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(6, 1)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(5, 3), Sq::from_xy(5, 4)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 1), Sq::from_xy(3, 3)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(9, 3), Sq::from_xy(9, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 1)),
    BookMovesEntry::new(Sq::from_xy(3, 5), Sq::from_xy(3, 6)),
];

const MOVES_MY_HISHAOCHI: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(7, 3), Sq::from_xy(7, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 3), Sq::from_xy(6, 4)),
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(6, 2), Sq::from_xy(6, 3)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(3, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(5, 3), Sq::from_xy(5, 4)),
    BookMovesEntry::new(Sq::from_xy(9, 3), Sq::from_xy(9, 4)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(8, 2), Sq::from_xy(7, 3)),
];

const MOVES_MY_NIMAIOCHI: &[BookMovesEntry] = &[
    BookMovesEntry::new(Sq::from_xy(6, 1), Sq::from_xy(7, 2)),
    BookMovesEntry::new(Sq::from_xy(3, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(5, 3), Sq::from_xy(5, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(5, 3)),
    BookMovesEntry::new(Sq::from_xy(4, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 3), Sq::from_xy(4, 4)),
    BookMovesEntry::new(Sq::from_xy(4, 2), Sq::from_xy(4, 3)),
    BookMovesEntry::new(Sq::from_xy(3, 3), Sq::from_xy(3, 4)),
    BookMovesEntry::new(Sq::from_xy(5, 1), Sq::from_xy(4, 2)),
    BookMovesEntry::new(Sq::from_xy(9, 3), Sq::from_xy(9, 4)),
    BookMovesEntry::new(Sq::from_xy(1, 3), Sq::from_xy(1, 4)),
    BookMovesEntry::new(Sq::from_xy(2, 1), Sq::from_xy(3, 3)),
    BookMovesEntry::new(Sq::from_xy(7, 1), Sq::from_xy(6, 2)),
    BookMovesEntry::new(Sq::from_xy(4, 4), Sq::from_xy(4, 5)),
];
