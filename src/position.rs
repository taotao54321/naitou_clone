use rand::Rng;

use crate::ai;
use crate::effect;
use crate::prelude::*;
use crate::sfen;
use crate::{Error, Result};

//--------------------------------------------------------------------
// 歩の筋
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PawnMask(u16);

impl PawnMask {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn from_board_side(board: &Board, side: Side) -> Self {
        let mask = (1..=9)
            .filter(|&x| (1..=9).any(|y| board[Sq::from_xy(x, y)].is_side_pt(side, Piece::Pawn)))
            .fold(0, |mask, x| mask | (1 << x));

        Self(mask)
    }

    pub fn test(&self, x: i32) -> bool {
        (self.0 & (1 << x)) != 0
    }

    pub fn set(&mut self, x: i32) {
        self.0 |= 1 << x;
    }
}

//--------------------------------------------------------------------
// MoveCmd (undo 用)
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MoveCmdNondrop {
    src: Sq,
    dst: Sq,
    is_promotion: bool,
    pt_capture: Option<Piece>,
}

impl MoveCmdNondrop {
    fn new(src: Sq, dst: Sq, is_promotion: bool, pt_capture: Option<Piece>) -> Self {
        Self {
            src,
            dst,
            is_promotion,
            pt_capture,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MoveCmdDrop {
    pt: Piece,
    dst: Sq,
}

impl MoveCmdDrop {
    fn new(pt: Piece, dst: Sq) -> Self {
        Self { pt, dst }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MoveCmd {
    Nondrop(MoveCmdNondrop),
    Drop(MoveCmdDrop),
}

impl MoveCmd {
    fn nondrop(src: Sq, dst: Sq, is_promotion: bool, pt_capture: Option<Piece>) -> Self {
        Self::Nondrop(MoveCmdNondrop::new(src, dst, is_promotion, pt_capture))
    }

    fn drop(pt: Piece, dst: Sq) -> Self {
        Self::Drop(MoveCmdDrop::new(pt, dst))
    }

    pub fn dst(&self) -> Sq {
        match self {
            Self::Nondrop(nondrop) => nondrop.dst,
            Self::Drop(drop) => drop.dst,
        }
    }

    pub fn pt_capture(&self) -> Option<Piece> {
        match self {
            Self::Nondrop(nondrop) => nondrop.pt_capture,
            Self::Drop(_) => None,
        }
    }
}

//--------------------------------------------------------------------
// 局面
//--------------------------------------------------------------------

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Position {
    side: Side,
    board: Board,
    hands: Hands,
    ply: i32,
}

impl Position {
    pub fn empty() -> Self {
        Self {
            side: Side::Sente,
            board: Board::empty(),
            hands: Hands::empty(),
            ply: 1,
        }
    }

    pub fn new(side: Side, board: Board, hands: Hands, ply: i32) -> Self {
        Self {
            side,
            board,
            hands,
            ply,
        }
    }

    pub fn from_sfen(sfen: impl AsRef<str>) -> crate::Result<Self> {
        sfen::sfen_to_position(sfen)
    }

    /// ランダムな局面を生成する。
    /// 互いの玉は必ず存在する。
    /// 互いの玉に王手が掛かっている局面も生成されうる。
    /// 手数は 1..=255 のいずれかとなる。
    pub fn random(rng: &mut impl Rng) -> Self {
        fn pick_sq(rng: &mut impl Rng, board: &Board, side: Side, pt: Piece) -> Option<Sq> {
            use rand::seq::IteratorRandom;
            Sq::iter_valid()
                .filter(|&sq| {
                    if !board[sq].is_empty() {
                        return false;
                    }
                    // 行きどころのない駒チェック
                    if !sq.can_put(side, pt) {
                        return false;
                    }
                    // 二歩チェック
                    if matches!(pt, Piece::Pawn) {
                        let pawn_mask = PawnMask::from_board_side(board, side);
                        if pawn_mask.test(sq.x().get()) {
                            return false;
                        }
                    }
                    true
                })
                .choose(rng)
        }

        const PROB_HAND: f64 = 0.2; // 各駒が持駒となる確率

        // とりあえず玉を除いた38枚も全て使うとする
        // (ただし二歩になる場合は捨てられる)
        let pts = [
            (Piece::Pawn, 18),
            (Piece::Lance, 4),
            (Piece::Knight, 4),
            (Piece::Silver, 4),
            (Piece::Bishop, 2),
            (Piece::Rook, 2),
            (Piece::Gold, 4),
        ]
        .iter()
        .flat_map(|(pt, n)| itertools::repeat_n(pt, *n))
        .copied();

        let mut board = Board::empty();
        let mut hands = Hands::empty();

        // 互いの玉を配置
        let sq_sente_king = pick_sq(rng, &board, Side::Sente, Piece::King).unwrap();
        board[sq_sente_king] = BoardCell::Sente(Piece::King);
        let sq_gote_king = pick_sq(rng, &board, Side::Gote, Piece::King).unwrap();
        board[sq_gote_king] = BoardCell::Gote(Piece::King);

        // 他の駒たちを配置
        for mut pt in pts {
            let side = Side::random(rng);
            if rng.gen_bool(PROB_HAND) {
                // 持駒
                hands[side][pt] += 1;
            } else {
                // 盤上
                // 成れる場合、50%の確率で成る
                if pt.can_promote() && rng.gen_bool(0.5) {
                    pt = pt.to_promoted().unwrap();
                }
                if let Some(sq) = pick_sq(rng, &board, side, pt) {
                    board[sq] = BoardCell::from_side_pt(side, pt);
                }
            }
        }

        let side = Side::random(rng);
        let ply = rng.gen_range(1, 256);

        Self {
            side,
            board,
            hands,
            ply,
        }
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn side_mut(&mut self) -> &mut Side {
        &mut self.side
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    pub fn hands(&self) -> &Hands {
        &self.hands
    }

    pub fn hands_mut(&mut self) -> &mut Hands {
        &mut self.hands
    }

    pub fn hand(&self, side: Side) -> &Hand {
        &self.hands[side]
    }

    pub fn hand_mut(&mut self, side: Side) -> &mut Hand {
        &mut self.hands[side]
    }

    pub fn ply(&self) -> i32 {
        self.ply
    }

    pub fn ply_mut(&mut self) -> &mut i32 {
        &mut self.ply
    }

    /// 手番側が敵玉を取れる状態かどうかを返す。
    pub fn can_capture_king(&self) -> bool {
        let sq = ai::find_king_sq(&self.board, self.side.inv()).unwrap();
        effect::iter_effects(&self.board, self.side).any(|(_, dst)| sq == dst)
    }

    /// mv の中身はある程度信用している。
    /// 特に、mv が pseudo-legal ならエラーにはならない。
    pub fn do_move(&mut self, mv: &Move) -> Result<MoveCmd> {
        let mv_cmd = match mv {
            Move::Nondrop(nondrop) => {
                let src = nondrop.src;
                let dst = nondrop.dst;
                let is_promotion = nondrop.is_promotion;

                let pt_src = self.board[src]
                    .piece_of(self.side)
                    .ok_or_else(|| Error::illegal_move(mv, "src is not my piece"))?;
                chk!(
                    !self.board[dst].is_side(self.side),
                    Error::illegal_move(mv, "dst is my piece")
                );
                let pt_dst = if is_promotion {
                    pt_src
                        .to_promoted()
                        .ok_or_else(|| Error::illegal_move(mv, "cannot promote"))?
                } else {
                    pt_src
                };
                let pt_capture = self.board[dst].piece_of(self.side.inv());

                self.board[src] = BoardCell::Empty;
                self.board[dst] = BoardCell::from_side_pt(self.side, pt_dst);
                if let Some(pt) = pt_capture {
                    self.hands[self.side][pt.to_raw()] += 1;
                }

                MoveCmd::nondrop(src, dst, is_promotion, pt_capture)
            }
            Move::Drop(drop) => {
                let pt = drop.pt;
                let dst = drop.dst;

                chk!(
                    self.hand(self.side)[pt] > 0,
                    Error::illegal_move(mv, "not in hand")
                );
                chk!(
                    self.board[dst].is_empty(),
                    Error::illegal_move(mv, "dst is not empty")
                );

                self.board[dst] = BoardCell::from_side_pt(self.side, pt);
                self.hands[self.side][pt] -= 1;

                MoveCmd::drop(pt, dst)
            }
        };

        self.side.toggle();
        self.ply += 1;

        Ok(mv_cmd)
    }

    /// mv_cmd の中身はある程度信用している。
    pub fn undo_move(&mut self, mv_cmd: &MoveCmd) -> Result<()> {
        let opponent = self.side.inv();

        match mv_cmd {
            MoveCmd::Nondrop(nondrop) => {
                let src = nondrop.src;
                let dst = nondrop.dst;
                let is_promotion = nondrop.is_promotion;
                let pt_capture = nondrop.pt_capture;

                let pt_dst = self.board[dst]
                    .piece_of(opponent)
                    .ok_or_else(|| Error::illegal_move_cmd(mv_cmd, "dst is not opponent piece"))?;
                chk!(
                    self.board[src].is_empty(),
                    Error::illegal_move_cmd(mv_cmd, "src is not empty")
                );
                let pt_src = if is_promotion {
                    pt_dst.to_raw()
                } else {
                    pt_dst
                };
                if let Some(pt) = pt_capture {
                    chk!(
                        self.hands[opponent][pt.to_raw()] > 0,
                        Error::illegal_move_cmd(mv_cmd, "not in hand")
                    );
                }

                self.board[src] = BoardCell::from_side_pt(opponent, pt_src);
                if let Some(pt) = pt_capture {
                    self.board[dst] = BoardCell::from_side_pt(self.side, pt);
                    self.hands[opponent][pt.to_raw()] -= 1;
                } else {
                    self.board[dst] = BoardCell::Empty;
                }
            }
            MoveCmd::Drop(drop) => {
                let pt = drop.pt;
                let dst = drop.dst;

                chk!(
                    self.board[dst] == BoardCell::from_side_pt(opponent, pt),
                    Error::illegal_move_cmd(mv_cmd, "dst mismatch")
                );

                self.board[dst] = BoardCell::Empty;
                self.hands[opponent][pt] += 1;
            }
        }

        self.side.toggle();
        self.ply -= 1;

        Ok(())
    }

    pub fn to_sfen(&self) -> String {
        sfen::position_to_sfen(self).into_owned()
    }
}
