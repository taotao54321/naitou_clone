//!===================================================================
//! 思考ルーチン
//!
//! 玉が取れるかどうかの判定はしばしば最大駒得/駒損マスのスコアを用いて行われる。
//! 閾値は原作通りにしている。
//!===================================================================

use std::cmp::Ordering;

use crate::book::{BookState, Formation};
use crate::effect::EffectBoard;
use crate::log::LoggerTrait;
use crate::my_move;
use crate::position::MoveCmd;
use crate::prelude::*;
use crate::price::{PRICES_0, PRICES_1, PRICES_2, PRICES_3};
use crate::record::RecordEntry;
use crate::util::{self, WrappingAddExt, WrappingSubExt};
use crate::your_move;

//--------------------------------------------------------------------
// 玉の位置
//--------------------------------------------------------------------

pub fn find_king_sq(board: &Board, side: Side) -> Option<Sq> {
    Sq::iter_valid().find(|&sq| board[sq].is_side_pt(side, Piece::King))
}

//--------------------------------------------------------------------
// 候補手とその付随情報
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandInfo {
    mv: Move,
    pt_src: Piece,             // 動かした/打った駒 src (成りの場合でもナマ駒)
    pt_dst: Piece,             // 動かした/打った駒 dst (成りの場合成駒)
    pt_capture: Option<Piece>, // 取った駒
    sq_king_my: Sq,            // 「ルート局面での」my 玉位置
    sq_king_your: Sq,          // 「ルート局面での」your 玉位置
}

impl CandInfo {
    fn from_pos_mv(pos: &Position, mv: &Move) -> Self {
        let my = pos.side();
        let your = my.inv();

        let (pt_src, pt_dst) = match mv {
            Move::Nondrop(nondrop) => {
                let pt_src = pos.board()[nondrop.src].piece_of(my).unwrap();
                let pt_dst = if nondrop.is_promotion {
                    pt_src.to_promoted().unwrap()
                } else {
                    pt_src
                };
                (pt_src, pt_dst)
            }
            Move::Drop(drop) => (drop.pt, drop.pt),
        };
        let pt_capture = pos.board()[mv.dst()].piece_of(your);

        let sq_king_my = find_king_sq(pos.board(), my).unwrap();
        let sq_king_your = find_king_sq(pos.board(), your).unwrap();

        Self {
            mv: mv.clone(),
            pt_src,
            pt_dst,
            pt_capture,
            sq_king_my,
            sq_king_your,
        }
    }
}

//--------------------------------------------------------------------
// 評価
//--------------------------------------------------------------------

/// root 局面の評価
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootEval {
    pub adv_price: u8,    // 最大駒得マスの your 駒の価値
    pub disadv_price: u8, // 最大駒損マスの my 駒の価値
    pub power_my: u8, // my 側: 8*(持飛+持角+成駒) + 4*(持金+持銀) + 2*(持桂+持香) + 1*(持歩) + (手数補正)
    pub power_your: u8, // your 側: 8*(持飛+持角+成駒) + 4*(持金+持銀) + 2*(持桂+持香) + 1*(持歩) + (手数補正)
    pub rbp_my: u8,     // (持飛数) + (持角数) + (成駒数)
}

/// 局面の評価
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionEval {
    pub adv_price: u8,          // 最大駒得マスの your 駒の価値 (駒得マスがない場合は 0)
    pub adv_sq: Sq,             // 最大駒得マス (ない場合は SQ_INVALID)
    pub disadv_price: u8,       // 最大駒損マスの my 駒の価値 (駒損マスがない場合は 0)
    pub disadv_sq: Sq,          // 最大駒得マス (ない場合は SQ_INVALID)
    pub hanging_your: bool,     // your 側の垂れ歩/垂れ香があるか
    pub king_safety_far_my: u8, // 自玉から距離 2 以下への my 駒利き数の総和
    pub king_threat_far_my: u8, // 自玉から距離 2 以下への your 駒利き数の総和
    pub king_threat_far_your: u8, // 敵玉から距離 2 以下への my 駒利き数の総和
    pub king_threat_near_my: u8, // 自玉からちょうど距離 1 への your 駒利き数の総和
    pub n_choke_my: u8, // 自玉からちょうど距離 1 で (your 駒利き数) >= (my 駒利き数) なるマスの数
    pub n_loose_my: u8, // my 側の離れ駒の数 (玉、桂、香、歩は対象外)
    pub n_promoted_my: u8, // my 側の成駒の数
    pub n_promoted_your: u8, // your 側の成駒の数
}

/// 候補手の評価
/// (*) の付いた項目は最善手との比較時にさまざまな基準により修正を受ける。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandEval {
    pub adv_price: u8,        // 最大駒得マスの your 駒の価値             (*)
    pub capture_price: u8,    // 取れる your 駒の価値 (取れない場合は 0)  (*)
    pub disadv_price: u8,     // 最大駒損マスの my 駒の価値               (*)
    pub dst_to_your_king: u8, // 移動先から your 玉への距離
    pub is_sacrifice: bool,   // タダ捨てか
    pub nega: u8,             // 駒損マスの my 駒の価値の総和             (*)
    pub posi: u8,             // 駒得マスの your 駒の価値の総和           (*)
    pub to_my_king: u8,       // nondrop の場合 dist(src, your玉), drop の場合 dist(dst, your玉)
}

/// 最善手の評価
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BestEval {
    pub adv_price: u8,
    pub adv_sq: Sq,
    pub capture_price: u8,
    pub disadv_price: u8,
    pub disadv_sq: Sq,
    pub dst_to_your_king: u8,
    pub king_safety_far_my: u8,
    pub king_threat_far_my: u8,
    pub king_threat_far_your: u8,
    pub n_loose_my: u8,
    pub n_promoted_my: u8,
    pub nega: u8,
    pub posi: u8,
    pub to_my_king: u8,
}

/// 必ずどれかの候補手は採用されるような初期値。
impl Default for BestEval {
    fn default() -> Self {
        Self {
            adv_price: 0,
            adv_sq: SQ_INVALID,
            capture_price: 0,
            disadv_price: 99,
            disadv_sq: SQ_INVALID,
            dst_to_your_king: 99,
            king_safety_far_my: 0,
            king_threat_far_my: 99,
            king_threat_far_your: 0,
            n_loose_my: 99,
            n_promoted_my: 0,
            nega: 99,
            posi: 0,
            to_my_king: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TweakResult {
    Normal,
    YourMate,
    Reject,
}

//--------------------------------------------------------------------
// 詰み判定
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
enum MateJudge {
    Nonmate,
    Mate,
    DropPawnMate,
}

//--------------------------------------------------------------------
// 原作における drop 時の src
//
// drop 候補手と最善手を比較する際、原作の駒種 ID 基準でより安い駒が優先される。
//--------------------------------------------------------------------

fn naitou_drop_src(pt: Piece) -> u8 {
    match pt {
        Piece::Rook => 207,
        Piece::Bishop => 206,
        Piece::Gold => 205,
        Piece::Silver => 204,
        Piece::Knight => 203,
        Piece::Lance => 202,
        Piece::Pawn => 201,
        _ => panic!("naitou_drop_src(): not hand piece: {:?}", pt),
    }
}

//--------------------------------------------------------------------
// undo 用
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepMyCmd {
    mv_cmd: Option<MoveCmd>,
    progress_ply: u8,
    progress_level: u8,
    progress_level_sub: u8,
    book_state: BookState,
    naitou_best_src: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveYourCmd {
    mv_cmd: MoveCmd,
    mv_your: Option<Move>,
    progress_ply: u8,
    progress_level: u8,
}

//--------------------------------------------------------------------
// 思考ルーチン
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ai {
    my: Side,
    pos: Position,
    mv_your: Option<Move>, // 直前の your 指し手
    progress_ply: u8,
    progress_level: u8,
    progress_level_sub: u8,
    book_state: BookState,

    // drop 候補手と最善手を比較する際に必要となる値。
    // 原作ではこの値が局面ごとに初期化されないため、状態を保持しておく必要がある。
    naitou_best_src: u8,
}

impl Ai {
    pub fn new(handicap: Handicap, timelimit: bool) -> Self {
        let my = handicap.my();
        let pos = handicap.initial_pos();

        let formation = Formation::from_handicap(handicap, timelimit);
        let book_state = BookState::new(formation);

        Self {
            my,
            pos,
            mv_your: None,
            progress_ply: 0,
            progress_level: 0,
            progress_level_sub: 0,
            book_state,

            naitou_best_src: 0,
        }
    }

    pub fn my(&self) -> Side {
        self.my
    }

    pub fn pos(&self) -> &Position {
        &self.pos
    }

    pub fn is_my_turn(&self) -> bool {
        self.pos.side() == self.my
    }

    pub fn is_your_turn(&self) -> bool {
        !self.is_my_turn()
    }

    pub fn progress_ply(&self) -> u8 {
        self.progress_ply
    }

    fn increment_progress_ply(&mut self) {
        self.progress_ply = std::cmp::min(100, self.progress_ply + 1);
    }

    pub fn progress_level(&self) -> u8 {
        self.progress_level
    }

    /// think(), move_my() を一括で行い、(RecordEntry, StepMyCmd) を返す。
    pub fn step_my<L: LoggerTrait>(&mut self, logger: &mut L) -> (RecordEntry, StepMyCmd) {
        let progress_ply = self.progress_ply;
        let progress_level = self.progress_level;
        let progress_level_sub = self.progress_level_sub;
        let book_state = self.book_state.clone();
        let naitou_best_src = self.naitou_best_src;

        let entry = self.think(logger);
        let mv_cmd = match &entry {
            RecordEntry::Move(mv) => Some(self.move_my(mv)),
            RecordEntry::MyWin(mv) => Some(self.move_my(mv)),
            _ => None,
        };

        let step_my_cmd = StepMyCmd {
            mv_cmd,
            progress_ply,
            progress_level,
            progress_level_sub,
            book_state,
            naitou_best_src,
        };

        (entry, step_my_cmd)
    }

    pub fn undo_step_my(&mut self, cmd_my: &StepMyCmd) {
        if let Some(mv_cmd) = &cmd_my.mv_cmd {
            self.pos.undo_move(mv_cmd).unwrap();
        }
        self.progress_ply = cmd_my.progress_ply;
        self.progress_level = cmd_my.progress_level;
        self.progress_level_sub = cmd_my.progress_level_sub;
        self.book_state = cmd_my.book_state.clone();
        self.naitou_best_src = cmd_my.naitou_best_src;
    }

    /// my 側の指し手を適用し、内部局面を更新する。
    /// think() で得られた指し手を与えることを想定している。
    pub fn move_my(&mut self, mv: &Move) -> MoveCmd {
        let my = self.my;
        assert_eq!(self.pos.side(), my);

        let mv_cmd = self.pos.do_move(mv).unwrap();
        self.increment_progress_ply();

        mv_cmd
    }

    /// your 側の指し手を適用し、内部局面を更新する。
    pub fn move_your(&mut self, mv: &Move) -> MoveYourCmd {
        let mv_your = self.mv_your.clone();
        let progress_ply = self.progress_ply;
        let progress_level = self.progress_level;

        let your = self.my.inv();
        assert_eq!(self.pos.side(), your);

        let mv_cmd = self.pos.do_move(mv).unwrap();
        self.mv_your = Some(mv.clone());
        self.increment_progress_ply();

        if self.progress_ply >= 51 {
            self.progress_level = std::cmp::min(2, self.progress_level + 1);
        }

        if self.progress_ply >= 71 {
            self.progress_level = 3;
        }

        MoveYourCmd {
            mv_cmd,
            mv_your,
            progress_ply,
            progress_level,
        }
    }

    pub fn undo_move_your(&mut self, cmd_your: &MoveYourCmd) {
        self.pos.undo_move(&cmd_your.mv_cmd).unwrap();
        self.mv_your = cmd_your.mv_your.clone();
        self.progress_ply = cmd_your.progress_ply;
        self.progress_level = cmd_your.progress_level;
    }

    pub fn think<L: LoggerTrait>(&mut self, logger: &mut L) -> RecordEntry {
        let my = self.my;

        let (entry, is_mate_your) = self.think_go(logger);

        let entry = match entry {
            RecordEntry::YourSuicide | RecordEntry::YourWin => entry,
            RecordEntry::Move(mv) => {
                let cmd = self.pos.do_move(&mv).unwrap();
                let eff_board = EffectBoard::from_board(self.pos.board(), my);
                let adv_price = self.eval_position(&eff_board, None).0.adv_price;
                self.pos.undo_move(&cmd).unwrap();

                if adv_price >= 31 && is_mate_your {
                    RecordEntry::MyWin(mv)
                } else {
                    RecordEntry::Move(mv)
                }
            }
            _ => unreachable!(),
        };

        logger.log_record_entry(entry.clone());
        entry
    }

    /// (思考結果, is_mate_your) を返す。
    /// 内部局面自体は更新しない。
    pub fn think_go<L: LoggerTrait>(&mut self, logger: &mut L) -> (RecordEntry, bool) {
        // 6 手目以前で必ず序盤処理を行う your 側指し手 (dst) たち (my 側が先手の場合)
        const DSTS_SPECIAL: &[Sq] = &[Sq::from_xy(4, 5), Sq::from_xy(5, 4), Sq::from_xy(2, 8)];

        let my = self.my;
        assert_eq!(self.pos.side(), my);

        logger.log_progress(
            self.progress_ply,
            self.progress_level,
            self.progress_level_sub,
        );
        logger.log_book_state(self.book_state.clone());

        let (mv_best, root_eval, best_eval, is_mate_your) = self.think_nonbook(logger);
        //dbg!(&mv_best, &root_eval, &best_eval);

        // 6 手目以前の特定の your 指し手に対しては必ず序盤処理を行う
        {
            let cond = self.progress_ply <= 6
                && self.mv_your.as_ref().map_or(false, |mv| {
                    DSTS_SPECIAL.iter().any(|dst| *dst == mv.dst().rel(my))
                });
            if cond && self.progress_level == 0 {
                let mv = self.process_opening();
                if let Some(mv) = mv {
                    return (RecordEntry::Move(mv), is_mate_your);
                }
                self.progress_level = 1;
            }
        }

        // 最大駒得/駒損マスのスコアを用いて勝敗判定
        if root_eval.adv_price >= 31 {
            return (RecordEntry::YourSuicide, is_mate_your);
        }
        if best_eval.disadv_price >= 31 {
            return (RecordEntry::YourWin, is_mate_your);
        }

        // この時点で mv_best は None ではないはず
        let mv_best = mv_best.unwrap();

        // 駒がぶつかったか?
        let nonquiet =
            root_eval.adv_price > 0 || root_eval.disadv_price > 0 || best_eval.capture_price > 0;

        // progress_level == 0 のとき、駒がぶつかるたびにサブ進行度を進める
        // サブ進行度が 5 になったら progress_level = 1 とする
        if self.progress_level == 0 && nonquiet {
            self.progress_level_sub += 1;
            if self.progress_level_sub >= 5 {
                self.progress_level = 1;
            }
        }

        // progress_level > 0 であるか、駒がぶつかったら序盤処理をスキップ
        if self.progress_level > 0 || nonquiet {
            return (RecordEntry::Move(mv_best), is_mate_your);
        }

        // posi 値によっては序盤処理をスキップ
        if best_eval.posi != best_eval.adv_price && best_eval.posi >= 8 {
            return (RecordEntry::Move(mv_best), is_mate_your);
        }

        // 序盤処理
        if self.progress_level == 0 {
            let mv = self.process_opening();
            if let Some(mv) = mv {
                return (RecordEntry::Move(mv), is_mate_your);
            }
            self.progress_level = 1;
        }

        (RecordEntry::Move(mv_best), is_mate_your)
    }

    /// 定跡手を取得する。
    /// 合法手かつ駒損のおそれがないかどうかの検査を行う。
    ///
    /// 戦型が Formation::Nothing になった場合、None を返す。
    fn process_opening(&mut self) -> Option<Move> {
        let my = self.my;
        let your = my.inv();

        let eff_board = EffectBoard::from_board(self.pos.board(), my);

        loop {
            let mv = self.book_state.process(&self.pos, self.progress_ply)?;

            // 非合法手はNG
            if !my_move::is_book_legal(&self.pos, &eff_board, &mv) {
                continue;
            }

            // 移動先の利きが my <= your ならNG
            if eff_board[mv.dst()][my].count() <= eff_board[mv.dst()][your].count() {
                continue;
            }

            // 駒損する手は基本的に弾く
            // ただし、6 手目以下で直前の your 移動先が my から見て６五の場合のみ許容する
            // これにより、your 先手で初手から 36歩、34歩、37桂、44歩、45桂、32銀、53桂不成 が実現する
            // 裏技的要素なのかも?
            let disadv = {
                let cmd_mv = self.pos.do_move(&mv).unwrap();
                let eff_board = EffectBoard::from_board(self.pos.board(), my);
                let res = self.eval_position(&eff_board, None).0.disadv_price > 0;
                self.pos.undo_move(&cmd_mv).unwrap();
                res
            };
            if disadv
                && self
                    .mv_your
                    .as_ref()
                    .map_or(true, |mv| mv.dst().rel(my) != Sq::from_xy(4, 5))
            {
                continue;
            }

            return Some(mv);
        }
    }

    /// 定跡を使わない思考。常に呼び出される。
    /// 自殺手は許されるが、打ち歩詰めは許されない。
    ///
    /// (最善手, ルート局面評価, 最善手評価, is_mate_your) を返す。
    /// ルート局面が既に勝ち(your 玉が取れる)であるか、指せる手がなければ最善手として None を返す。
    fn think_nonbook<L: LoggerTrait>(
        &mut self,
        logger: &mut L,
    ) -> (Option<Move>, RootEval, BestEval, bool) {
        let my = self.my;

        let eff_board = EffectBoard::from_board(self.pos.board(), my);
        logger.log_root_eff_board(eff_board.clone());

        let root_eval = self.eval_root(&eff_board);
        logger.log_root_eval(root_eval.clone());

        let mut best_eval = BestEval::default();

        // your 玉が取れるかどうかの判定。
        if root_eval.adv_price >= 30 {
            return (None, root_eval, best_eval, false); // この場合 is_mate_your は false (原作通り)
        }

        let mut mv_best = None;
        let mut is_mate_your = false;

        let cands: Vec<_> = my_move::moves_pseudo_legal(&self.pos).collect();
        for mv_cand in cands {
            logger.start_cand(mv_cand.clone());

            let cand = CandInfo::from_pos_mv(&self.pos, &mv_cand);

            let (improved, cand_is_mate_your, _pos_eval, _cand_eval) =
                self.try_improve_best(&root_eval, &mut best_eval, &cand, logger);

            if improved {
                logger.log_cand_improve();
            }
            logger.end_cand();

            if improved || cand_is_mate_your {
                self.update_naitou_best_src(&mv_cand);
                mv_best = Some(mv_cand);
            }
            if cand_is_mate_your {
                is_mate_your = true;
                break;
            }
        }

        logger.log_best_eval(best_eval.clone());

        (mv_best, root_eval, best_eval, is_mate_your)
    }

    /// ルート局面評価
    pub fn eval_root(&self, eff_board: &EffectBoard) -> RootEval {
        let my = self.my;
        let your = my.inv();

        let pos_eval = self.eval_position(eff_board, None).0;
        let (rbp_my, power_my) = self.eval_power(&self.pos, my, pos_eval.n_promoted_my);
        let power_your = self.eval_power(&self.pos, your, pos_eval.n_promoted_your).1;

        RootEval {
            adv_price: pos_eval.adv_price,
            disadv_price: pos_eval.disadv_price,
            power_my,
            power_your,
            rbp_my,
        }
    }

    /// (rbp, power) を返す。
    /// 理論上オーバーフローがありうることに注意。
    fn eval_power(&self, pos: &Position, side: Side, n_promoted: u8) -> (u8, u8) {
        let rbp: u8 = pos.hand(side)[Piece::Rook] + pos.hand(side)[Piece::Bishop] + n_promoted;
        let gs: u8 = pos.hand(side)[Piece::Gold] + pos.hand(side)[Piece::Silver];
        let kl: u8 = pos.hand(side)[Piece::Knight] + pos.hand(side)[Piece::Lance];
        let p: u8 = pos.hand(side)[Piece::Pawn];

        // 手数補正 (77 手目以降かどうかで係数が変わる)
        let mut ply_factor = self.progress_ply / 11;
        if ply_factor >= 7 {
            ply_factor *= 2;
        }

        let mut power: u8 = 0;
        power.wadd(rbp.wrapping_mul(8));
        power.wadd(4 * gs);
        power.wadd(2 * kl);
        power.wadd(p);
        power.wadd(ply_factor);

        (rbp, power)
    }

    /// 現局面を評価する。
    /// cand が与えられた場合、末端局面とみなし、CandEval も返す。
    pub fn eval_position(
        &self,
        eff_board: &EffectBoard,
        cand: Option<&CandInfo>,
    ) -> (PositionEval, Option<CandEval>) {
        let my = self.my;
        let your = my.inv();

        let (sq_king_my, sq_king_your) = if let Some(cand) = cand {
            (cand.sq_king_my, cand.sq_king_your)
        } else {
            (
                find_king_sq(self.pos.board(), my).unwrap(),
                find_king_sq(self.pos.board(), your).unwrap(),
            )
        };

        let (posi, adv_price, adv_sq) = self.eval_adv(&self.pos, eff_board);
        let (nega, disadv_price, disadv_sq, is_sacrifice_my) =
            self.eval_disadv(&self.pos, eff_board, cand);
        let hanging_your = self.eval_hanging(self.pos.board(), eff_board);
        let n_loose_my = self.eval_n_loose(self.pos.board(), eff_board);
        let (n_promoted_my, n_promoted_your) = self.eval_n_promoted(self.pos.board());
        let (
            king_safety_far_my,
            king_threat_far_my,
            king_threat_far_your,
            king_threat_near_my,
            n_choke_my,
        ) = self.eval_around_kings(eff_board, sq_king_my, sq_king_your);

        let pos_eval = PositionEval {
            adv_price,
            adv_sq,
            disadv_price,
            disadv_sq,
            hanging_your,
            king_safety_far_my,
            king_threat_far_my,
            king_threat_far_your,
            king_threat_near_my,
            n_choke_my,
            n_loose_my,
            n_promoted_my,
            n_promoted_your,
        };

        let cand_eval = cand.map(|cand| {
            let capture_price = cand.pt_capture.map_or(0, |pt| PRICES_0[pt]);
            let dst_to_your_king = Sq::dist(cand.mv.dst(), sq_king_your).unwrap() as u8;
            let to_my_king = match &cand.mv {
                Move::Nondrop(nondrop) => Sq::dist(nondrop.src, sq_king_my).unwrap(),
                Move::Drop(drop) => Sq::dist(drop.dst, sq_king_my).unwrap(),
            } as u8;
            CandEval {
                adv_price,
                capture_price,
                disadv_price,
                dst_to_your_king,
                is_sacrifice: is_sacrifice_my,
                nega,
                posi,
                to_my_king,
            }
        });

        (pos_eval, cand_eval)
    }

    /// 駒得マスの評価
    ///
    /// mv_your: 直前の your 指し手
    ///
    /// (駒得マスの your 駒価値の総和, 最大駒得マスの価値, 最大駒得マス) を返す。
    fn eval_adv(&self, pos: &Position, eff_board: &EffectBoard) -> (u8, u8, Sq) {
        let my = self.my;
        let your = my.inv();

        let mut sum_price = 0u8;
        let mut adv_price = 0;
        let mut adv_sq = SQ_INVALID;

        let sqs = Sq::iter_valid_sim(my).filter(|&sq| self.is_adv_sq(pos, eff_board, sq));
        for sq in sqs {
            let pt_your = pos.board()[sq].piece_of(your).unwrap();
            let price = PRICES_1[pt_your];

            sum_price.wadd(price); // 原作通り
            if util::chmax(&mut adv_price, price) {
                adv_sq = sq;
            }
        }

        (sum_price, adv_price, adv_sq)
    }

    fn is_adv_sq(&self, pos: &Position, eff_board: &EffectBoard, sq: Sq) -> bool {
        let my = self.my;
        let your = my.inv();

        // your 駒がなければ駒得マスではない
        let pt_your = unwrap_or_return!(pos.board()[sq].piece_of(your), false);

        let eff_my = eff_board[sq][my].count();
        let eff_your = eff_board[sq][your].count();
        match (eff_my, eff_your) {
            (0, _) => false, // my 利きがなければ駒得マスではない
            (_, 0) => true,  // my 利きがあり、your 利きがなければ駒得マス
            _ => {
                // 両者の効きがある場合、駒価値比較と進行度で判定
                let atk_my = eff_board[sq][my].attacker().unwrap();
                let price_my = PRICES_1[atk_my];
                let price_your = PRICES_1[pt_your];

                if price_my < price_your {
                    true
                } else if price_my == price_your {
                    self.progress_level != 0
                } else {
                    false
                }
            }
        }
    }

    /// 駒損マスの評価
    ///
    /// (nega, disadv_price, disadv_sq, is_sacrifice_my) を返す。
    fn eval_disadv(
        &self,
        pos: &Position,
        eff_board: &EffectBoard,
        cand: Option<&CandInfo>,
    ) -> (u8, u8, Sq, bool) {
        let my = self.my;

        let mut nega = 0u8;
        let mut disadv_price = 0;
        let mut disadv_sq = SQ_INVALID;
        let mut is_sacrifice_my = false;

        // 取り返しフラグ
        // ある駒損マスにおいて (my効き) >= (your効き) かつ (my駒の価値) > (your attackerの価値)
        // のとき true となる。
        // いったんこのフラグが立つと、「それ以降全ての駒損マスにおいて」nega, disadv_price に補正
        // がかかる。
        let mut exchange = false; // 取り返しフラグ

        for sq in Sq::iter_valid_sim(my) {
            let (is_disadv, exchange_enable) = self.is_disadv_sq(pos, eff_board, sq);
            if !is_disadv {
                continue;
            }
            if exchange_enable {
                exchange = true;
            }

            // sq が cand の移動先で、かつ cand が駒取りでなければ駒捨てフラグを立てる
            if let Some(cand) = cand {
                if sq == cand.mv.dst() && cand.pt_capture.is_none() {
                    is_sacrifice_my = true;
                }
            }

            let pt_my = pos.board()[sq].piece_of(my).unwrap();
            let price = PRICES_3[pt_my];

            nega.wadd(price); // 原作通り
            if util::chmax(&mut disadv_price, price) {
                disadv_sq = sq;
            }

            // 取り返しフラグが立っているとき、nega, disadv_price 補正(原作通り)
            if exchange {
                nega.wsub(1);
                disadv_price.wsub(1);
            }
        }

        (nega, disadv_price, disadv_sq, is_sacrifice_my)
    }

    /// (駒損マスかどうか, 取り返しフラグ) を返す。
    fn is_disadv_sq(&self, pos: &Position, eff_board: &EffectBoard, sq: Sq) -> (bool, bool) {
        let my = self.my;
        let your = my.inv();

        // my 駒がなければ駒損マスではない
        let pt_my = unwrap_or_return!(pos.board()[sq].piece_of(my), (false, false));

        let eff_my = eff_board[sq][my].count();
        let eff_your = eff_board[sq][your].count();

        // your 利きがなければ駒損マスではない
        if eff_your == 0 {
            return (false, false);
        }

        // my 駒が玉ならば駒損マス(王手が掛かっている)
        if matches!(pt_my, Piece::King) {
            return (true, false);
        }

        // your 利きがあり、my 利きがなければ駒損マス
        if eff_my == 0 {
            return (true, false);
        }
        // 両者の利きがある場合、利き数および駒価値を比較して判定

        let atk_my = eff_board[sq][my].attacker().unwrap();
        let atk_your = eff_board[sq][your].attacker().unwrap();
        let price_pt_my = PRICES_3[pt_my];
        let price_atk_my = PRICES_3[atk_my];
        let price_atk_your = PRICES_2[atk_your];

        if eff_my < eff_your {
            (price_pt_my + price_atk_my >= price_atk_your, false)
        } else {
            // eff_my >= eff_your かつ price_pt_my > price_atk_your のケースは駒損マスとするが、
            // 利きが同数以上なら取り返しが利くため、専用フラグを立てる。
            if price_pt_my > price_atk_your {
                (true, true)
            } else {
                (false, false)
            }
        }
    }

    /// 垂れ歩/垂れ香判定
    fn eval_hanging(&self, board: &Board, eff_board: &EffectBoard) -> bool {
        let my = self.my;
        let your = my.inv();

        // 自陣4段目までに存在する歩or香の1つ先のマスで my 側の利きが負けていたら成立
        Sq::iter_valid().any(|sq| {
            if sq.y().rel(my).get() < 6 {
                return false;
            }
            board[sq].piece_of(your).map_or(false, |pt| {
                if !matches!(pt, Piece::Pawn | Piece::Lance) {
                    return false;
                }
                let dst = sq + 11 * my.sgn();
                eff_board[dst][my].count() < eff_board[dst][your].count()
            })
        })
    }

    /// 離れ駒カウント
    fn eval_n_loose(&self, board: &Board, eff_board: &EffectBoard) -> u8 {
        let my = self.my;

        let mut n_loose_my = 0;

        for sq in Sq::iter_valid() {
            if let Some(pt) = board[sq].piece_of(my) {
                if matches!(pt, Piece::King | Piece::Knight | Piece::Lance | Piece::Pawn) {
                    continue;
                }
                if eff_board[sq][my].count() == 0 {
                    n_loose_my += 1;
                }
            }
        }

        n_loose_my
    }

    /// 成駒カウント
    fn eval_n_promoted(&self, board: &Board) -> (u8, u8) {
        let mut n_promoted = [0, 0];

        for sq in Sq::iter_valid() {
            let cell = &board[sq];

            for side in Side::iter() {
                let idx = if side == self.my { 0 } else { 1 };
                if cell.piece_of(side).map_or(false, |pt| pt.is_promoted()) {
                    n_promoted[idx] += 1;
                }
            }
        }

        (n_promoted[0], n_promoted[1])
    }

    /// 互いの玉周りの安全度/危険度評価
    fn eval_around_kings(
        &self,
        eff_board: &EffectBoard,
        sq_king_my: Sq,
        sq_king_your: Sq,
    ) -> (u8, u8, u8, u8, u8) {
        let my = self.my;
        let your = my.inv();

        let mut king_safety_far_my = 0;
        let mut king_threat_far_my = 0;
        let mut king_threat_far_your = 0;
        let mut king_threat_near_my = 0;
        let mut n_choke_my = 0;

        for sq in Sq::iter_valid() {
            let cell = &eff_board[sq];
            let dist_to_my = Sq::dist(sq, sq_king_my).unwrap();
            let dist_to_your = Sq::dist(sq, sq_king_your).unwrap();

            if dist_to_my <= 2 {
                king_safety_far_my += cell[my].count();
                king_threat_far_my += cell[your].count();
            }

            if dist_to_my == 1 {
                king_threat_near_my += cell[your].count();
                if cell[your].count() >= cell[my].count() {
                    n_choke_my += 1;
                }
            }

            if dist_to_your <= 2 {
                king_threat_far_your += cell[my].count();
            }
        }

        (
            king_safety_far_my,
            king_threat_far_my,
            king_threat_far_your,
            king_threat_near_my,
            n_choke_my,
        )
    }

    /// ルート局面評価、現在の最善手、候補手を与え、最善手更新が可能か判定する。
    /// mv_best, best_eval を更新し、(improved, is_mate_your, pos_eval, cand_eval) を返す。
    ///
    /// 内部で候補手を適用して元に戻す操作を行う。
    fn try_improve_best<L: LoggerTrait>(
        &mut self,
        root_eval: &RootEval,
        best_eval: &mut BestEval,
        cand: &CandInfo,
        logger: &mut L,
    ) -> (bool, bool, PositionEval, CandEval) {
        let my = self.my;

        let cmd_cand = self.pos.do_move(&cand.mv).unwrap();

        let eff_board = EffectBoard::from_board(self.pos.board(), my);
        logger.log_cand_eff_board(eff_board.clone());

        let (pos_eval, cand_eval) = self.eval_position(&eff_board, Some(cand));
        let mut cand_eval = cand_eval.unwrap();

        logger.log_cand_pos_eval(pos_eval.clone());
        logger.log_cand_eval(cand_eval.clone());

        let tweak_res = self.tweak_eval(root_eval, &pos_eval, &mut cand_eval, cand, logger);

        self.pos.undo_move(&cmd_cand).unwrap();

        let mut is_mate_your = false;
        match tweak_res {
            TweakResult::Reject => return (false, false, pos_eval, cand_eval),
            TweakResult::YourMate => {
                is_mate_your = true;
            }
            _ => {}
        }

        let improved = self.can_improve_best(root_eval, &pos_eval, &cand_eval, best_eval, &cand.mv);
        if improved {
            best_eval.adv_price = cand_eval.adv_price;
            best_eval.adv_sq = pos_eval.adv_sq;
            best_eval.capture_price = cand_eval.capture_price;
            best_eval.disadv_price = cand_eval.disadv_price;
            best_eval.disadv_sq = pos_eval.disadv_sq;
            best_eval.dst_to_your_king = cand_eval.dst_to_your_king;
            best_eval.king_safety_far_my = pos_eval.king_safety_far_my;
            best_eval.king_threat_far_my = pos_eval.king_threat_far_my;
            best_eval.king_threat_far_your = pos_eval.king_threat_far_your;
            best_eval.n_loose_my = pos_eval.n_loose_my;
            best_eval.n_promoted_my = pos_eval.n_promoted_my;
            best_eval.nega = cand_eval.nega;
            best_eval.posi = cand_eval.posi;
            best_eval.to_my_king = cand_eval.to_my_king;
        }

        (improved, is_mate_your, pos_eval, cand_eval)
    }

    /// 様々な要素を勘案して候補手の評価値を修正する。
    fn tweak_eval<L: LoggerTrait>(
        &mut self,
        root_eval: &RootEval,
        pos_eval: &PositionEval,
        cand_eval: &mut CandEval,
        cand: &CandInfo,
        logger: &mut L,
    ) -> TweakResult {
        macro_rules! log_cand_eval {
            () => {
                logger.log_cand_eval(cand_eval.clone());
            };
        }

        let my = self.my;

        let sq_king_my = cand.sq_king_my;
        let sq_king_your = cand.sq_king_your;

        let mut is_mate_your = false;

        // 以下の条件を満たすとき your 玉の詰み判定を行う:
        //
        //   * my 玉が取られない
        //   * your 玉に王手が掛かっている
        //   * 候補手の移動先から your 玉への距離が 3 未満
        //
        // 詰みの有無にかかわらず次の段階へ進むが、打ち歩詰めはこの段階で却下する。
        if cand_eval.disadv_price < 30
            && cand_eval.adv_price >= 30
            && cand_eval.dst_to_your_king < 3
        {
            match self.judge_mate_your(&cand.mv) {
                MateJudge::Nonmate => {}
                MateJudge::DropPawnMate => return TweakResult::Reject,
                MateJudge::Mate => {
                    // 詰ます手は明らかに最善なので、他の候補手に上書きされないよう評価値を細工
                    is_mate_your = true;
                    cand_eval.adv_price = 60;
                    cand_eval.capture_price = 60;
                    cand_eval.disadv_price = 0;
                }
            }
        }
        log_cand_eval!();

        // 評価値修正パート
        // オーバーフローが起こりうるので注意

        // 大きな駒損をせず歩(不成)で駒を取る手をプラス評価
        if cand_eval.disadv_price < 20
            && matches!(cand.pt_dst, Piece::Pawn)
            && cand_eval.capture_price > 0
        {
            cand_eval.nega.wsub(1);
        }
        log_cand_eval!();

        // 原則として駒捨ては却下 (王手対応や詰ます手は除く)
        if cand_eval.is_sacrifice && root_eval.disadv_price < 30 && !is_mate_your {
            return TweakResult::Reject;
        }
        log_cand_eval!();

        // your 側の垂れ歩/香が存在すればマイナス評価
        if pos_eval.hanging_your {
            cand_eval.nega.wadd(4);
        }
        log_cand_eval!();

        // 中盤以降は自玉から遠い歩を取られるのを軽視
        if (root_eval.power_my >= 15 || root_eval.power_your >= 15)
            && cand_eval.nega < 3
            && Sq::dist(pos_eval.disadv_sq, sq_king_my).unwrap() >= 4
        {
            cand_eval.nega.wsub(cand_eval.disadv_price);
        }
        log_cand_eval!();

        // 終盤用追加処理
        if root_eval.power_my >= 25 || root_eval.power_your >= 25 {
            // 互いの玉から遠い最大駒得マスの評価を下げる
            if Sq::dist(pos_eval.adv_sq, sq_king_my).unwrap() >= 3
                && Sq::dist(pos_eval.adv_sq, sq_king_your).unwrap() >= 4
            {
                cand_eval.posi.wsub(cand_eval.adv_price);
            }
            log_cand_eval!();

            // 互いの玉から遠い桂香を取られるのを軽視
            if cand_eval.disadv_price < 7
                && Sq::dist(pos_eval.disadv_sq, sq_king_my).unwrap() >= 3
                && Sq::dist(pos_eval.disadv_sq, sq_king_your).unwrap() >= 3
            {
                cand_eval.nega.wsub(cand_eval.disadv_price);
            }
            log_cand_eval!();

            // your 玉近くの駒を取る手の評価を上げる
            // 互いの玉から遠い駒を取る手の評価を下げる
            if cand_eval.capture_price > 0 {
                let dst_to_my_king = Sq::dist(cand.mv.dst(), sq_king_my).unwrap();
                let dst_to_your_king = Sq::dist(cand.mv.dst(), sq_king_your).unwrap();
                if dst_to_your_king <= 2 {
                    cand_eval.capture_price.wadd(2);
                } else if dst_to_my_king >= 4 && dst_to_your_king >= 4 {
                    cand_eval.capture_price.wsub(3);
                }
            }
        }
        log_cand_eval!();

        // 寄せが見込めない状況で無闇に王手を掛けないようにする
        // ただし「王手xx取り」ならOK
        if cand_eval.adv_price >= 30
            && pos_eval.king_threat_far_your < 12
            && root_eval.rbp_my < 4
            && root_eval.power_my < 35
            && cand_eval.posi.wrapping_sub(cand_eval.adv_price) < 3
        {
            cand_eval.posi.wsub(cand_eval.adv_price);
        }
        log_cand_eval!();

        // 高い駒を自陣側かつ my 玉から遠くに打つ手の評価を下げる (合駒は除く)
        if cand.mv.is_drop()
            && matches!(
                cand.pt_dst,
                Piece::Rook | Piece::Bishop | Piece::Gold | Piece::Silver
            )
            && cand.mv.dst().y().rel(my).get() >= 5
            && root_eval.disadv_price < 30
            && cand_eval.dst_to_your_king >= 3
            && cand_eval.to_my_king >= 3
        {
            cand_eval.nega.wadd(2);
        }
        log_cand_eval!();

        // 意図がよくわからない
        if root_eval.power_my >= 27 {
            if (3..6).contains(&cand_eval.posi) {
                cand_eval.capture_price.wadd(1);
            } else if (6..).contains(&cand_eval.posi) {
                cand_eval.capture_price.wadd(4);
            }
        }
        log_cand_eval!();

        // 大駒を打つ手は敵陣側ほど評価を高くする (合駒の場合はペナルティなし)
        if cand.mv.is_drop() && matches!(cand.pt_dst, Piece::Rook | Piece::Bishop) {
            let y_rel = cand.mv.dst().y().rel(my).get();
            if y_rel <= 2 {
                cand_eval.posi.wadd(2);
                cand_eval.nega.wsub(2);
            } else if root_eval.disadv_price < 30 {
                cand_eval.posi.wsub(2);
                cand_eval.nega.wadd(2);
                if y_rel >= 6 {
                    cand_eval.nega.wadd(2);
                }
            }
        }
        log_cand_eval!();

        // 玉で駒を取る手は評価を下げる(なるべく他の駒で取る)
        if matches!(cand.pt_dst, Piece::King) {
            cand_eval.capture_price.wsub(1);
            cand_eval.posi.wsub(2);
        }
        log_cand_eval!();

        // 意図がよくわからない
        // 最後の条件は sq_king_your を誤って sq_king_my にした疑惑もある
        if root_eval.power_my >= 31
            && cand_eval.adv_price < 4
            && cand_eval.disadv_price == 0
            && pos_eval.king_threat_far_your >= 7
            && Sq::dist(pos_eval.adv_sq, sq_king_my).unwrap() <= 2
        {
            cand_eval.posi.wadd((pos_eval.king_threat_far_your - 7) / 2);
        }
        log_cand_eval!();

        // 自分から角をぶつける手を避ける意図?
        if cand_eval.adv_price == 16 && matches!(cand.pt_dst, Piece::Bishop) {
            cand_eval.posi.wsub(cand_eval.adv_price);
            cand_eval.adv_price = 0;
        }
        log_cand_eval!();

        // 戦力が豊富かつ自玉が危険なら大駒を温存せず直ちに使う意図?
        if root_eval.power_my >= 27
            && !(cand.mv.is_drop() && matches!(cand.pt_dst, Piece::Rook | Piece::Bishop))
        {
            cand_eval.posi.wsub(4 * pos_eval.n_choke_my);
            cand_eval.nega.wadd(4 * pos_eval.n_choke_my);
        }
        log_cand_eval!();

        // 意図がよくわからない
        if cand_eval.capture_price >= 8
            && cand.pt_capture.map_or(false, |pt| {
                matches!(
                    pt,
                    Piece::King | Piece::Rook | Piece::Bishop | Piece::Gold | Piece::Silver
                )
            })
            && (cand_eval.adv_price >= 30 || Sq::dist(pos_eval.adv_sq, sq_king_your).unwrap() < 3)
        {
            if root_eval.power_my >= 30
                && pos_eval.king_threat_far_your >= 7
                && root_eval.rbp_my >= 4
            {
                cand_eval.posi.wadd(2);
                if (8..30).contains(&cand_eval.disadv_price) {
                    cand_eval.nega = 8;
                    cand_eval.disadv_price = 8;
                }
            }
        }
        log_cand_eval!();

        // 自玉が危険な場合、玉で駒を取るのは価値なしとする
        //
        // XXX: 原作ではこの部分に配列外参照バグがあるが、そこまでは再現していない。
        if pos_eval.king_threat_near_my >= 5 && matches!(cand.pt_dst, Piece::King) {
            cand_eval.capture_price = 0;
        }
        log_cand_eval!();

        // 戦力が豊富なら駒を取りながらの王手の評価を上げる
        if root_eval.power_my >= 35 && cand_eval.adv_price >= 30 && cand_eval.capture_price >= 2 {
            cand_eval.nega.wsub(2);
        }
        log_cand_eval!();

        // 意図がよくわからない
        if root_eval.power_my >= 20 && cand_eval.capture_price < 2 {
            match cand_eval.posi {
                0..=4 => {}
                5..=9 => cand_eval.capture_price.wadd(1),
                10..=19 => cand_eval.capture_price.wadd(2),
                _ => cand_eval.capture_price.wadd(3),
            }
        }
        log_cand_eval!();

        // 飛/角を敵陣以外に打つ手の評価を下げる
        if cand.mv.is_drop()
            && matches!(cand.pt_dst, Piece::Rook | Piece::Bishop)
            && cand.mv.dst().y().rel(my).get() >= 4
        {
            cand_eval.posi.wsub(3);
            cand_eval.nega.wadd(3);
        }
        log_cand_eval!();

        // 成駒を動かす場合、your 玉に近づく手の方を高く評価する
        if let Move::Nondrop(nondrop) = &cand.mv {
            if cand.pt_src.is_promoted() {
                let dd = Sq::dist(nondrop.src, sq_king_your).unwrap()
                    - Sq::dist(nondrop.dst, sq_king_your).unwrap();
                cand_eval.posi.wadd(dd as u8);
            }
        }
        log_cand_eval!();

        // 戦力が豊富なら王手の評価を上げる
        if root_eval.power_my >= 25 && cand_eval.adv_price >= 30 {
            cand_eval.posi.wadd(4);
            cand_eval.capture_price.wadd(1);
            cand_eval.nega.wsub(2);
        }
        log_cand_eval!();

        // 高い駒を取りながらの王手の評価を上げる
        if cand_eval.adv_price >= 30 && cand_eval.capture_price >= 8 {
            cand_eval.nega.wsub(4);
        }
        log_cand_eval!();

        // 負の評価値を 0 に補正
        let chmax_zero = |x: &mut u8| {
            if *x & 0x80 != 0 {
                *x = 0;
            }
        };
        chmax_zero(&mut cand_eval.capture_price);
        chmax_zero(&mut cand_eval.posi);
        chmax_zero(&mut cand_eval.nega);
        log_cand_eval!();

        if is_mate_your {
            TweakResult::YourMate
        } else {
            TweakResult::Normal
        }
    }

    /// 候補手が最善手より優れているか?
    fn can_improve_best(
        &self,
        root_eval: &RootEval,
        pos_eval: &PositionEval,
        cand_eval: &CandEval,
        best_eval: &BestEval,
        mv_cand: &Move,
    ) -> bool {
        macro_rules! tie_break {
            ($lhs:expr, $rhs:expr) => {
                match $lhs.cmp(&$rhs) {
                    Ordering::Greater => return true,
                    Ordering::Less => return false,
                    Ordering::Equal => {}
                }
            };
        }

        // cand, best のいずれか一方のみが自殺手なら自殺手でない方を採用
        if cand_eval.disadv_price >= 40 && best_eval.disadv_price < 40 {
            return false;
        }
        if cand_eval.disadv_price < 40 && best_eval.disadv_price >= 40 {
            return true;
        }

        match cand_eval.nega.cmp(&best_eval.nega) {
            Ordering::Greater => match cand_eval.capture_price.cmp(&best_eval.capture_price) {
                Ordering::Less => return false,
                Ordering::Greater => {
                    let dcapture = cand_eval.capture_price - best_eval.capture_price;
                    let dnega = cand_eval.nega - best_eval.nega;
                    return dnega <= dcapture;
                }
                Ordering::Equal => {
                    if root_eval.power_my < 18 {
                        return false;
                    }
                    if cand_eval.capture_price > 0 {
                        return false;
                    }
                    match cand_eval.posi.cmp(&best_eval.posi) {
                        Ordering::Greater => {
                            let dposi = cand_eval.posi - best_eval.posi;
                            let dnega = cand_eval.nega - best_eval.nega;
                            return dnega < dposi;
                        }
                        _ => return false,
                    }
                }
            },
            Ordering::Less => {
                if (30..80).contains(&best_eval.nega) {
                    return true;
                }

                match cand_eval.capture_price.cmp(&best_eval.capture_price) {
                    Ordering::Greater => return true,
                    Ordering::Less => {
                        let dcapture = best_eval.capture_price - cand_eval.capture_price;
                        let dnega = best_eval.nega - cand_eval.nega;
                        tie_break!(dnega, dcapture);
                    }
                    Ordering::Equal => {
                        if root_eval.power_my < 18 {
                            return true;
                        }
                        if cand_eval.capture_price > 0 {
                            return true;
                        }
                        match cand_eval.posi.cmp(&best_eval.posi) {
                            Ordering::Greater | Ordering::Equal => return true,
                            Ordering::Less => {
                                let dposi = best_eval.posi - cand_eval.posi;
                                let dnega = best_eval.nega - cand_eval.nega;
                                tie_break!(dnega, dposi);
                            }
                        }
                    }
                }
            }
            Ordering::Equal => tie_break!(cand_eval.capture_price, best_eval.capture_price),
        }

        // タイブレーク

        tie_break!(pos_eval.n_promoted_my, best_eval.n_promoted_my);
        tie_break!(cand_eval.posi, best_eval.posi);
        tie_break!(cand_eval.adv_price, best_eval.adv_price);

        match mv_cand {
            Move::Nondrop(_) => {
                tie_break!(
                    pos_eval.king_threat_far_your,
                    best_eval.king_threat_far_your
                );
                tie_break!(pos_eval.king_safety_far_my, best_eval.king_safety_far_my);
                tie_break!(best_eval.king_threat_far_my, pos_eval.king_threat_far_my);
                tie_break!(best_eval.n_loose_my, pos_eval.n_loose_my);
                if cand_eval.to_my_king >= 3 {
                    tie_break!(best_eval.dst_to_your_king, cand_eval.dst_to_your_king);
                }
                cand_eval.to_my_king > best_eval.to_my_king
            }
            Move::Drop(drop) => {
                // 合駒以外では nondrop を優先
                if root_eval.disadv_price < 30 {
                    return false;
                }
                // より安い駒を打つ手なら採用、さもなくば却下
                // ここでは原作における駒種 ID で比較
                let naitou_cand_src = naitou_drop_src(drop.pt);
                naitou_cand_src < self.naitou_best_src
            }
        }
    }

    /// your 玉の詰み判定。
    /// 王手回避手を順次試す。
    fn judge_mate_your(&mut self, mv_cand: &Move) -> MateJudge {
        let my = self.my;
        let your = my.inv();

        let mvs: Vec<_> = your_move::moves_evasion(&self.pos).collect();
        for mv in mvs {
            let cmd = self.pos.do_move(&mv).unwrap();
            let eff_board = EffectBoard::from_board(self.pos.board(), my);
            let sq_king_your = find_king_sq(self.pos.board(), your).unwrap();
            self.pos.undo_move(&cmd).unwrap();

            // your 玉に my 利きがなければ詰みを逃れている
            if eff_board[sq_king_your][my].count() == 0 {
                return MateJudge::Nonmate;
            }
        }
        // この時点で詰み/打ち歩詰めのいずれか

        if mv_cand.is_drop_pt(Piece::Pawn) {
            return MateJudge::DropPawnMate;
        }

        MateJudge::Mate
    }

    fn update_naitou_best_src(&mut self, mv: &Move) {
        match mv {
            Move::Nondrop(_) => self.naitou_best_src = 200, // drop 比較できればよいのでこれで十分
            Move::Drop(drop) => self.naitou_best_src = naitou_drop_src(drop.pt),
        }
    }
}
