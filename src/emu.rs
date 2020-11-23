//!===================================================================
//! エミュレータ (FCEUX) 上での操作
//!===================================================================

use std::path::Path;

use once_cell::sync::Lazy;

use fceux::MemoryDomain;

use crate::ai::{BestEval, CandEval, PositionEval, RootEval};
use crate::book::{BookState, Formation};
use crate::effect::{EffectBoard, EffectInfo};
use crate::prelude::*;
use crate::util;
use crate::{Error, Result};

//--------------------------------------------------------------------
// util
//--------------------------------------------------------------------

const fn bit_test(x: u8, bit: i32) -> bool {
    (x & (1 << bit)) != 0
}

const fn bit_set(x: u8, bit: i32) -> u8 {
    x | (1 << bit)
}

//--------------------------------------------------------------------
// Buttons
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Buttons(u8);

#[allow(non_snake_case)]
impl Buttons {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn value(&self) -> u8 {
        self.0
    }

    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub const fn A(&self) -> bool {
        bit_test(self.0, 0)
    }
    pub const fn B(&self) -> bool {
        bit_test(self.0, 1)
    }
    pub const fn S(&self) -> bool {
        bit_test(self.0, 2)
    }
    pub const fn T(&self) -> bool {
        bit_test(self.0, 3)
    }
    pub const fn U(&self) -> bool {
        bit_test(self.0, 4)
    }
    pub const fn D(&self) -> bool {
        bit_test(self.0, 5)
    }
    pub const fn L(&self) -> bool {
        bit_test(self.0, 6)
    }
    pub const fn R(&self) -> bool {
        bit_test(self.0, 7)
    }

    pub const fn setA(self) -> Self {
        Self(bit_set(self.0, 0))
    }
    pub const fn setB(self) -> Self {
        Self(bit_set(self.0, 1))
    }
    pub const fn setS(self) -> Self {
        Self(bit_set(self.0, 2))
    }
    pub const fn setT(self) -> Self {
        Self(bit_set(self.0, 3))
    }
    pub const fn setU(self) -> Self {
        Self(bit_set(self.0, 4))
    }
    pub const fn setD(self) -> Self {
        Self(bit_set(self.0, 5))
    }
    pub const fn setL(self) -> Self {
        Self(bit_set(self.0, 6))
    }
    pub const fn setR(self) -> Self {
        Self(bit_set(self.0, 7))
    }
}

pub const BTNS_NONE: Buttons = Buttons::new();
pub const BTNS_A: Buttons = BTNS_NONE.setA();
pub const BTNS_B: Buttons = BTNS_NONE.setB();
pub const BTNS_S: Buttons = BTNS_NONE.setS();
pub const BTNS_T: Buttons = BTNS_NONE.setT();
pub const BTNS_U: Buttons = BTNS_NONE.setU();
pub const BTNS_D: Buttons = BTNS_NONE.setD();
pub const BTNS_L: Buttons = BTNS_NONE.setL();
pub const BTNS_R: Buttons = BTNS_NONE.setR();

pub const BTNS_UL: Buttons = BTNS_NONE.setU().setL();
pub const BTNS_UR: Buttons = BTNS_NONE.setU().setR();
pub const BTNS_DL: Buttons = BTNS_NONE.setD().setL();
pub const BTNS_DR: Buttons = BTNS_NONE.setD().setR();

//--------------------------------------------------------------------
// Cursor
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Cursor {
    Board(Sq),
    Hand(Piece),
}

impl Cursor {
    pub fn board(sq: Sq) -> Self {
        Self::Board(sq)
    }

    pub fn hand(pt: Piece) -> Self {
        assert!(pt.is_hand());
        Self::Hand(pt)
    }
}

//--------------------------------------------------------------------
// Traveller
//
// カーソル位置間の最短経路キャッシュ。
//
// カーソル位置は盤上と持駒を合わせて 88 通りある。
// 頂点 0..=80 を盤上に、81..=87 を持駒に割り当てたグラフとして実装。
//
// マスは your 側から見たものになることに注意。
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct TravellerEntry {
    size: u8,
    seq: [Buttons; 11], // 最も離れた頂点間でも 11 回で移動可能
}

impl Default for TravellerEntry {
    fn default() -> Self {
        Self {
            size: 0,
            seq: [BTNS_NONE; 11],
        }
    }
}

#[derive(Debug)]
pub struct Traveller {
    entrys: [[TravellerEntry; 88]; 88],
}

impl Traveller {
    fn new() -> Self {
        const VERTEX_INVALID: usize = 999;
        const INF: u8 = 100;

        let graph = Self::graph();

        // i から j へ最短経路で行くときの (次に辿るべき頂点, 操作)
        // i==j なら (i, BTNS_NONE)
        // 到達不能なら (VERTEX_INVALID, BTNS_NONE)
        let mut nxt = [[(VERTEX_INVALID, BTNS_NONE); 88]; 88];
        for i in 0..88 {
            for j in 0..88 {
                if i == j {
                    nxt[i][j] = (i, BTNS_NONE);
                    continue;
                }
                let btns = graph[i][j];
                if !btns.is_empty() {
                    nxt[i][j] = (j, btns);
                }
            }
        }

        let mut dist = [[0; 88]; 88];
        for i in 0..88 {
            for j in 0..88 {
                if i == j {
                    dist[i][j] = 0;
                } else {
                    dist[i][j] = if graph[i][j].is_empty() { INF } else { 1 };
                }
            }
        }

        // Warshall-Floyd
        for k in 0..88 {
            for i in 0..88 {
                if dist[i][k] == INF {
                    continue;
                }
                for j in 0..88 {
                    if dist[k][j] == INF {
                        continue;
                    }
                    let d_new = dist[i][k] + dist[k][j];
                    if util::chmin(&mut dist[i][j], d_new) {
                        nxt[i][j] = nxt[i][k];
                    }
                }
            }
        }

        // 経路復元
        let mut entrys = [[TravellerEntry::default(); 88]; 88];
        for src in 0..88 {
            for dst in 0..88 {
                assert_ne!(dist[src][dst], INF);
                let entry = &mut entrys[src][dst];
                let mut v = src;
                while v != dst {
                    let (nxt_v, nxt_btns) = nxt[v][dst];
                    entry.seq[entry.size as usize] = nxt_btns;
                    entry.size += 1;
                    v = nxt_v;
                }
            }
        }

        Self { entrys }
    }

    /// i から j へ移動するための操作のテーブルを返す。
    /// (i==j, または移動できないときは BTNS_NONE)
    fn graph() -> [[Buttons; 88]; 88] {
        // (dy, dx, btns)
        const NEIGHBORS: [(i32, i32, Buttons); 8] = [
            (-1, -1, BTNS_UL),
            (-1, 0, BTNS_U),
            (-1, 1, BTNS_UR),
            (0, -1, BTNS_L),
            (0, 1, BTNS_R),
            (1, -1, BTNS_DL),
            (1, 0, BTNS_D),
            (1, 1, BTNS_DR),
        ];

        let mut graph = [[BTNS_NONE; 88]; 88];

        // 盤上のマス間の接続
        for y in 1..=9 {
            for x in 1..=9 {
                for &(dy, dx, btns) in NEIGHBORS.iter() {
                    let yy = y + dy;
                    let xx = x + dx;
                    if yy < 1 || 9 < yy || xx < 1 || 9 < xx {
                        continue;
                    }
                    let i = Self::vertex_xy(x, y);
                    let j = Self::vertex_xy(xx, yy);
                    graph[i][j] = btns;
                }
            }
        }

        // 盤から持駒エリアへの接続
        for y in 1..=9 {
            let i = Self::vertex_xy(9, y);
            graph[i][Self::vertex_hand(Piece::Rook)] = BTNS_R;
            graph[i][Self::vertex_hand(Piece::Silver)] = BTNS_DR;
        }

        // 持駒エリアから盤への接続
        {
            let i = Self::vertex_hand(Piece::Rook);
            graph[i][Self::vertex_xy(9, 6)] = BTNS_UL;
            graph[i][Self::vertex_xy(9, 7)] = BTNS_L;
            graph[i][Self::vertex_xy(9, 8)] = BTNS_DL;
        }
        {
            let i = Self::vertex_hand(Piece::Silver);
            graph[i][Self::vertex_xy(9, 7)] = BTNS_UL;
            graph[i][Self::vertex_xy(9, 8)] = BTNS_L;
            graph[i][Self::vertex_xy(9, 9)] = BTNS_DL;
        }
        {
            let i = Self::vertex_hand(Piece::Pawn);
            graph[i][Self::vertex_xy(9, 8)] = BTNS_UL;
            graph[i][Self::vertex_xy(9, 9)] = BTNS_L;
        }

        // 持駒エリア内の接続
        {
            let mut add_edge = |pt1: Piece, pt2: Piece, btns: Buttons| {
                let i = Self::vertex_hand(pt1);
                let j = Self::vertex_hand(pt2);
                graph[i][j] = btns;
            };

            add_edge(Piece::Rook, Piece::Bishop, BTNS_R);
            add_edge(Piece::Rook, Piece::Silver, BTNS_D);
            add_edge(Piece::Rook, Piece::Knight, BTNS_DR);

            add_edge(Piece::Bishop, Piece::Rook, BTNS_L);
            add_edge(Piece::Bishop, Piece::Gold, BTNS_R);
            add_edge(Piece::Bishop, Piece::Silver, BTNS_DL);
            add_edge(Piece::Bishop, Piece::Knight, BTNS_D);
            add_edge(Piece::Bishop, Piece::Lance, BTNS_DR);

            add_edge(Piece::Gold, Piece::Bishop, BTNS_L);
            add_edge(Piece::Gold, Piece::Silver, BTNS_R);
            add_edge(Piece::Gold, Piece::Knight, BTNS_DL);
            add_edge(Piece::Gold, Piece::Lance, BTNS_D);

            add_edge(Piece::Silver, Piece::Rook, BTNS_U);
            add_edge(Piece::Silver, Piece::Bishop, BTNS_UR);
            add_edge(Piece::Silver, Piece::Knight, BTNS_R);
            add_edge(Piece::Silver, Piece::Pawn, BTNS_D);

            add_edge(Piece::Knight, Piece::Rook, BTNS_UL);
            add_edge(Piece::Knight, Piece::Bishop, BTNS_U);
            add_edge(Piece::Knight, Piece::Gold, BTNS_UR);
            add_edge(Piece::Knight, Piece::Silver, BTNS_L);
            add_edge(Piece::Knight, Piece::Lance, BTNS_R);
            add_edge(Piece::Knight, Piece::Pawn, BTNS_DL);

            add_edge(Piece::Lance, Piece::Bishop, BTNS_UL);
            add_edge(Piece::Lance, Piece::Gold, BTNS_U);
            add_edge(Piece::Lance, Piece::Knight, BTNS_L);
            add_edge(Piece::Lance, Piece::Pawn, BTNS_R);

            add_edge(Piece::Pawn, Piece::Silver, BTNS_U);
        }

        graph
    }

    pub fn vertex_sq(sq: Sq) -> usize {
        debug_assert!(sq.is_valid());
        let x = sq.x().get() as usize;
        let y = sq.y().get() as usize;
        9 * (y - 1) + (x - 1)
    }

    pub fn vertex_xy(x: i32, y: i32) -> usize {
        Self::vertex_sq(Sq::from_xy(x, y))
    }

    pub fn vertex_hand(pt: Piece) -> usize {
        match pt {
            Piece::Rook => 81,
            Piece::Bishop => 82,
            Piece::Gold => 83,
            Piece::Silver => 84,
            Piece::Knight => 85,
            Piece::Lance => 86,
            Piece::Pawn => 87,
            _ => panic!("invalid piece: {:?}", pt),
        }
    }

    pub fn vertex_cursor(cursor: &Cursor) -> usize {
        match cursor {
            Cursor::Board(sq) => Self::vertex_sq(*sq),
            Cursor::Hand(pt) => Self::vertex_hand(*pt),
        }
    }

    pub fn query(&self, i: usize, j: usize) -> &[Buttons] {
        let entry = &self.entrys[i][j];
        &entry.seq[0..entry.size as usize]
    }
}

pub static TRAVELLER: Lazy<Traveller> = Lazy::new(Traveller::new);

//--------------------------------------------------------------------
// emu
//--------------------------------------------------------------------

// 次フレームから your 側の入力待ちループに入る
pub const ADDR_YOUR_TURN: u16 = 0xCEFC;

pub const ADDR_THINK: u16 = 0xEF70;

pub const ADDR_ROOT_EVALED: u16 = 0xF03E;

pub const ADDR_TRY_IMPROVE_BEST: u16 = 0xF282;
pub const ADDR_IMPROVE_BEST: u16 = 0xF6F9;
pub const ADDR_TRY_IMPROVE_BEST_DONE_NONDROP: u16 = 0xF0F8;
pub const ADDR_TRY_IMPROVE_BEST_DONE_DROP: u16 = 0xF25C;

pub const ADDR_THINK_DONE: u16 = 0xDD0A;

pub const ADDR_YOUR_SUICIDE: u16 = 0xDD44;
pub const ADDR_YOUR_WIN: u16 = 0xDD47;
pub const ADDR_MOVE_MY: u16 = 0xDFD3;
pub const ADDR_MY_WIN: u16 = 0xDFD6;

pub const ADDRS_TWEAK: &[u16] = &[
    0xF2AC, 0xF2C5, 0xF2E9, 0xF2F7, 0xF329, 0xF364, 0xF395, 0xF3D0, 0xF3FA, 0xF423, 0xF441, 0xF475,
    0xF48C, 0xF4DC, 0xF4FC, 0xF521, 0xF57A, 0xF590, 0xF5AB, 0xF5D1, 0xF5F1, 0xF623, 0xF643, 0xF65A,
    0xF674,
];

pub fn decode_sq(value: u8) -> Sq {
    if value == 99 {
        return SQ_INVALID;
    }

    match get_my() {
        Side::Sente => Sq::new(value.into()).inv(),
        Side::Gote => Sq::new(value.into()),
    }
}

pub fn encode_sq(sq: Sq) -> u8 {
    if sq == SQ_INVALID {
        return 99;
    }

    match get_my() {
        Side::Sente => sq.inv().get() as u8,
        Side::Gote => sq.get() as u8,
    }
}

pub fn decode_pt(value: u8) -> Option<Piece> {
    match value {
        1 => Some(Piece::King),
        2 => Some(Piece::Rook),
        3 => Some(Piece::Bishop),
        4 => Some(Piece::Gold),
        5 => Some(Piece::Silver),
        6 => Some(Piece::Knight),
        7 => Some(Piece::Lance),
        8 => Some(Piece::Pawn),
        9 => Some(Piece::Dragon),
        10 => Some(Piece::Horse),
        12 => Some(Piece::ProSilver),
        13 => Some(Piece::ProKnight),
        14 => Some(Piece::ProLance),
        15 => Some(Piece::ProPawn),
        _ => None,
    }
}

pub fn decode_pt_my(value: u8) -> Option<Piece> {
    decode_pt(value - 15)
}

pub fn decode_pt_your(value: u8) -> Option<Piece> {
    decode_pt(value)
}

pub fn decode_my_move(src_value: u8, dst_value: u8, is_promotion: bool) -> Move {
    let dst = decode_sq(dst_value);

    match src_value {
        201 => Move::drop(Piece::Pawn, dst),
        202 => Move::drop(Piece::Lance, dst),
        203 => Move::drop(Piece::Knight, dst),
        204 => Move::drop(Piece::Silver, dst),
        205 => Move::drop(Piece::Gold, dst),
        206 => Move::drop(Piece::Bishop, dst),
        207 => Move::drop(Piece::Rook, dst),
        _ => {
            let src = decode_sq(src_value);
            Move::nondrop(src, dst, is_promotion)
        }
    }
}

pub fn decode_your_move(src_value: u8, dst_value: u8, is_promotion: bool) -> Move {
    let dst = decode_sq(dst_value);

    match src_value {
        213 => Move::drop(Piece::Rook, dst),
        214 => Move::drop(Piece::Bishop, dst),
        215 => Move::drop(Piece::Gold, dst),
        216 => Move::drop(Piece::Silver, dst),
        217 => Move::drop(Piece::Knight, dst),
        218 => Move::drop(Piece::Lance, dst),
        219 => Move::drop(Piece::Pawn, dst),
        _ => {
            let src = decode_sq(src_value);
            Move::nondrop(src, dst, is_promotion)
        }
    }
}

pub fn init(path_rom: impl AsRef<Path>) -> Result<()> {
    fceux::init(path_rom).map_err(|e| Error::Emu(format!("fceux::init() failed: {}", e)))
}

pub fn run_frame_hooked(buttons: Buttons, f: &dyn FnMut(u16)) {
    fceux::run_frame(buttons.value(), 0, |_, _| {}, f);
}

pub fn run_frames_hooked(n: i32, buttons: Buttons, f: &dyn FnMut(u16)) {
    for _ in 0..n {
        run_frame_hooked(buttons, f);
    }
}

pub fn run_frame(buttons: Buttons) {
    run_frame_hooked(buttons, &|_| {});
}

pub fn run_frames(n: i32, buttons: Buttons) {
    for _ in 0..n {
        run_frame(buttons);
    }
}

pub fn read(addr: u16) -> u8 {
    fceux::mem_read(addr, MemoryDomain::Cpu)
}

pub fn get_handicap() -> Handicap {
    match read(0xFE) {
        1 => Handicap::YourSente,
        2 => Handicap::YourHishaochi,
        3 => Handicap::YourNimaiochi,
        4 => Handicap::MySente,
        5 => Handicap::MyHishaochi,
        6 => Handicap::MyNimaiochi,
        x => panic!("invalid handicap: {}", x),
    }
}

pub fn get_my() -> Side {
    get_handicap().my()
}

pub fn get_your() -> Side {
    get_my().inv()
}

pub fn is_my_turn() -> bool {
    read(0x77) == 0
}

pub fn is_your_turn() -> bool {
    !is_my_turn()
}

pub fn get_side() -> Side {
    if is_my_turn() {
        get_my()
    } else {
        get_your()
    }
}

pub fn get_board() -> Board {
    let my = get_my();
    let your = my.inv();

    let mut board = Board::empty();

    for sq in Sq::iter_valid() {
        let cell_my = read(0x49B + u16::from(encode_sq(sq)));
        let cell_your = read(0x3A9 + u16::from(encode_sq(sq)));

        let cell = if (cell_my, cell_your) == (0, 0) {
            Some(BoardCell::Empty)
        } else if cell_my == 0 {
            decode_pt_your(cell_your).map(|pt| BoardCell::from_side_pt(your, pt))
        } else if cell_your == 0 {
            decode_pt_my(cell_my).map(|pt| BoardCell::from_side_pt(my, pt))
        } else {
            None
        }
        .expect(&format!("invalid cell: your={}, my={}", cell_your, cell_my));

        board[sq] = cell;
    }

    board
}

pub fn get_hand_my() -> Hand {
    let mut hand = Hand::empty();

    hand[Piece::Rook] = read(0x594 + 0);
    hand[Piece::Bishop] = read(0x594 + 1);
    hand[Piece::Gold] = read(0x594 + 2);
    hand[Piece::Silver] = read(0x594 + 3);
    hand[Piece::Knight] = read(0x594 + 4);
    hand[Piece::Lance] = read(0x594 + 5);
    hand[Piece::Pawn] = read(0x594 + 6);

    hand
}

pub fn get_hand_your() -> Hand {
    let mut hand = Hand::empty();

    hand[Piece::Rook] = read(0x58D + 0);
    hand[Piece::Bishop] = read(0x58D + 1);
    hand[Piece::Gold] = read(0x58D + 2);
    hand[Piece::Silver] = read(0x58D + 3);
    hand[Piece::Knight] = read(0x58D + 4);
    hand[Piece::Lance] = read(0x58D + 5);
    hand[Piece::Pawn] = read(0x58D + 6);

    hand
}

pub fn get_hands() -> Hands {
    let hand_my = get_hand_my();
    let hand_your = get_hand_your();

    if get_my().is_gote() {
        Hands::new(hand_your, hand_my)
    } else {
        Hands::new(hand_my, hand_your)
    }
}

pub fn get_ply() -> i32 {
    let lo = read(0x15);
    let hi = read(0x16);
    100 * i32::from(hi) + i32::from(lo)
}

pub fn get_position() -> Position {
    Position::new(get_side(), get_board(), get_hands(), get_ply())
}

pub fn get_effect_board() -> EffectBoard {
    let my = get_my();
    let your = my.inv();

    let mut eff_board = EffectBoard::empty();

    for sq in Sq::iter_valid() {
        let count_my = read(0x514 + u16::from(encode_sq(sq)));
        let attacker_my = read(0x1F9 + u16::from(encode_sq(sq)));
        let attacker_my = if attacker_my == 99 {
            None
        } else {
            Some(decode_pt_my(attacker_my).expect(&format!("invalid attacker_my: {}", attacker_my)))
        };

        let count_your = read(0x422 + u16::from(encode_sq(sq)));
        let attacker_your = read(0x180 + u16::from(encode_sq(sq)));
        let attacker_your = if attacker_your == 99 {
            None
        } else {
            Some(
                decode_pt_your(attacker_your)
                    .expect(&format!("invalid attacker_your: {}", attacker_your)),
            )
        };

        eff_board[sq][my] = EffectInfo::new(count_my, attacker_my);
        eff_board[sq][your] = EffectInfo::new(count_your, attacker_your);
    }

    eff_board
}

pub fn get_my_move() -> Move {
    let src_value = read(0x5BC);
    let dst_value = read(0x5BB);
    let is_promotion = read(0x5C0) != 0;

    decode_my_move(src_value, dst_value, is_promotion)
}

pub fn get_your_move() -> Move {
    let src_value = read(0x5A2);
    let dst_value = read(0x5A1);
    let is_promotion = read(0x5BF) != 0;

    decode_your_move(src_value, dst_value, is_promotion)
}

pub fn get_cand_move() -> Move {
    let src_value = read(0x277);
    let dst_value = read(0x276);
    let is_promotion = read(0x279) != 0;

    decode_my_move(src_value, dst_value, is_promotion)
}

pub fn get_best_move() -> Move {
    let src_value = read(0x285);
    let dst_value = read(0x284);
    let is_promotion = read(0x28C) != 0;

    decode_my_move(src_value, dst_value, is_promotion)
}

pub fn get_progress_ply() -> u8 {
    read(0x5C1)
}

pub fn get_progress_level() -> u8 {
    read(0x28E)
}

pub fn get_progress_level_sub() -> u8 {
    read(0x5C8)
}

pub fn get_formation() -> Formation {
    match read(0x5BE) {
        0 => Formation::Nakabisha,
        1 => Formation::Sikenbisha,
        3 => Formation::Kakugawari,
        4 => Formation::Sujichigai,
        6 => Formation::YourHishaochi,
        7 => Formation::YourNimaiochi,
        8 => Formation::MyHishaochi,
        9 => Formation::MyNimaiochi,
        99 => Formation::Nothing,
        x => panic!("invalid formation: {}", x),
    }
}

pub fn get_book_state() -> BookState {
    let formation = get_formation();
    let done_branch = (0..16).fold(0, |acc, i| {
        if read(0x2C + i) != 0 {
            acc | (1 << i)
        } else {
            acc
        }
    });
    let done_moves = (0..24).fold(0, |acc, i| {
        if read(0x3C + i) != 0 {
            acc | (1 << i)
        } else {
            acc
        }
    });

    BookState {
        formation,
        done_branch,
        done_moves,
    }
}

pub fn get_root_eval() -> RootEval {
    RootEval {
        adv_price: read(0x280),
        disadv_price: read(0x282),
        power_my: read(0x5E4),
        power_your: read(0x5E7),
        rbp_my: read(0x5EA),
    }
}

pub fn get_position_eval() -> PositionEval {
    PositionEval {
        adv_price: read(0x272),
        adv_sq: decode_sq(read(0x273)),
        disadv_price: read(0x274),
        disadv_sq: decode_sq(read(0x275)),
        hanging_your: read(0x5DF) != 0,
        king_safety_far_my: read(0x295),
        king_threat_far_my: read(0x296),
        king_threat_far_your: read(0x299),
        king_threat_near_my: read(0x5EB),
        n_choke_my: read(0x5E5),
        n_loose_my: read(0x297),
        n_promoted_my: read(0x293),
        n_promoted_your: read(0x5E8),
    }
}

pub fn get_cand_eval() -> CandEval {
    CandEval {
        adv_price: read(0x272),
        capture_price: read(0x278),
        disadv_price: read(0x274),
        dst_to_your_king: read(0x294),
        is_sacrifice: read(0x27C) != 0,
        nega: read(0x5E0),
        posi: read(0x2A4),
        to_my_king: read(0x298),
    }
}

pub fn get_best_eval() -> BestEval {
    BestEval {
        adv_price: read(0x286),
        adv_sq: decode_sq(read(0x287)),
        capture_price: read(0x28A),
        disadv_price: read(0x288),
        disadv_sq: decode_sq(read(0x289)),
        dst_to_your_king: read(0x29B),
        king_safety_far_my: read(0x29C),
        king_threat_far_my: read(0x29D),
        king_threat_far_your: read(0x2A0),
        n_loose_my: read(0x29E),
        n_promoted_my: read(0x29A),
        nega: read(0x5E2),
        posi: read(0x2A6),
        to_my_king: read(0x29F),
    }
}

pub fn get_cursor() -> Cursor {
    let x = read(0xD6);
    let y = read(0xD7);

    match (x, y) {
        (1..=9, y) => Cursor::Board(Sq::from_xy(x.into(), y.into())),
        (10, 3) => Cursor::Hand(Piece::Rook),
        (10, 4) => Cursor::Hand(Piece::Bishop),
        (10, 5) => Cursor::Hand(Piece::Gold),
        (10, 6) => Cursor::Hand(Piece::Silver),
        (10, 7) => Cursor::Hand(Piece::Knight),
        (10, 8) => Cursor::Hand(Piece::Lance),
        (10, 9) => Cursor::Hand(Piece::Pawn),
        _ => panic!("invalid cursor: x={}, y={}", x, y),
    }
}

pub fn start_game(handicap: Handicap, timelimit: bool) {
    let select_count = match handicap {
        Handicap::YourSente => 0,
        Handicap::YourHishaochi => 2,
        Handicap::YourNimaiochi => 4,
        Handicap::MySente => 6,
        Handicap::MyHishaochi => 8,
        Handicap::MyNimaiochi => 10,
    } + if timelimit { 1 } else { 0 };

    fceux::power();

    run_frames(10, BTNS_NONE);

    for _ in 0..select_count {
        run_frame(BTNS_S);
        run_frame(BTNS_NONE);
    }

    run_frame(BTNS_T);
}

/// your 側の指し手を実行する。
/// 着手から 20 フレームほど演出が入るので、この過程で思考ルーチンが実行されることはない。
pub fn move_your(mv: &Move, your: Side) {
    fn move_cursor(src: &Cursor, dst: &Cursor, interval: i32) {
        let i = Traveller::vertex_cursor(src);
        let j = Traveller::vertex_cursor(dst);
        let seq = TRAVELLER.query(i, j);
        for &btns in seq {
            run_frames(3, btns);
            run_frames(interval, BTNS_NONE);
        }
    }

    let promotable = match mv {
        Move::Nondrop(nondrop) => {
            let pt = get_board()[nondrop.src()].piece_of(your).unwrap();
            can_promote(your, pt, nondrop.src(), nondrop.dst())
        }
        Move::Drop(_) => false,
    };

    let src = match mv {
        Move::Nondrop(nondrop) => Cursor::board(nondrop.src.rel(your)),
        Move::Drop(drop) => Cursor::hand(drop.pt),
    };
    let dst = Cursor::board(mv.dst().rel(your));

    // あまり高速に入力すると認識されないことがあるので、若干余裕を持たせている

    move_cursor(&get_cursor(), &src, 3);

    run_frames(3, BTNS_A);
    run_frames(4, BTNS_NONE);

    move_cursor(&src, &dst, 3);

    run_frames(3, BTNS_A);

    if promotable {
        run_frames(3, BTNS_NONE);
        if !mv.is_promotion() {
            run_frames(3, BTNS_D);
            run_frames(3, BTNS_NONE);
        }
        run_frames(3, BTNS_A);
    }
}
