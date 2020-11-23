//!===================================================================
//! 内藤九段将棋秘伝 (FC) シミュレータ
//!
//! 盤面は周囲を幅1の壁で囲った 11*11 マス。
//! マスを2次元座標 (x,y) で表す場合、x=0 が左端、y=0 が上端となる。
//!
//! マスなどは符号付き数で表現する。符号なしだと相対インデックスなどの扱いが面倒なため。
//!
//! マスが盤面 11*11 内にあるとき、ok であるという。
//! マスが将棋盤 9*9 内にあるとき、valid であるという。
//!
//! マスなどの値のチェックは重大なバグに繋がりそうな部分のみで行っている。
//!
//! "my" は思考ルーチン側、"your" は対戦相手側の意。
//!===================================================================

use either::Either;

#[macro_use]
mod util;

pub mod ai;
pub mod book;
pub mod effect;
pub mod log;
pub mod my_move;
pub mod position;
pub mod prelude;
pub mod pretty;
pub mod price;
pub mod record;
pub mod sfen;
pub mod usi;
pub mod usi_random;
pub mod your_move;
pub mod your_player;

#[cfg(feature = "emu")]
pub mod emu;

use position::Position;

//--------------------------------------------------------------------
// util
//--------------------------------------------------------------------

/// Sq などに対し整数を加算/減算できるようにする。
/// 相対インデックスを用いた移動などに使う。
macro_rules! impl_add_sub {
    ($t:ty) => {
        impl ::std::ops::Add<i32> for $t {
            type Output = $t;

            fn add(self, rhs: i32) -> Self::Output {
                Self(self.0 + rhs)
            }
        }

        impl ::std::ops::AddAssign<i32> for $t {
            fn add_assign(&mut self, rhs: i32) {
                self.0 += rhs;
            }
        }

        impl ::std::ops::Sub<i32> for $t {
            type Output = $t;

            fn sub(self, rhs: i32) -> Self::Output {
                Self(self.0 - rhs)
            }
        }

        impl ::std::ops::SubAssign<i32> for $t {
            fn sub_assign(&mut self, rhs: i32) {
                self.0 -= rhs;
            }
        }
    };
}

pub fn can_promote(side: Side, pt: Piece, src: Sq, dst: Sq) -> bool {
    pt.can_promote() && (src.can_promote(side) || dst.can_promote(side))
}

//--------------------------------------------------------------------
// エラー
//--------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("illegal move: {1}: {0:?}")]
    IllegalMove(Move, String),

    #[error("illegal move command: {1}: {0:?}")]
    IllegalMoveCmd(position::MoveCmd, String),

    #[error("invalid sfen: {0}")]
    InvalidSfen(String),

    #[error("invalid usi command: {0}")]
    InvalidUsiCmd(String),

    #[error("record parse error: {0}")]
    RecordParseError(String),

    #[error("emulation error: {0}")]
    Emu(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn illegal_move(mv: &Move, msg: impl Into<String>) -> Self {
        Self::IllegalMove(mv.clone(), msg.into())
    }

    pub fn illegal_move_cmd(mv_cmd: &position::MoveCmd, msg: impl Into<String>) -> Self {
        Self::IllegalMoveCmd(mv_cmd.clone(), msg.into())
    }

    pub fn invalid_sfen(msg: impl Into<String>) -> Self {
        Self::InvalidSfen(msg.into())
    }

    pub fn invalid_usi_cmd(msg: impl Into<String>) -> Self {
        Self::InvalidUsiCmd(msg.into())
    }

    pub fn record_parse_error(msg: impl Into<String>) -> Self {
        Self::RecordParseError(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

//--------------------------------------------------------------------
// 手番
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Side {
    Sente,
    Gote,
}

impl Side {
    pub fn random(rng: &mut impl rand::Rng) -> Self {
        use rand::seq::SliceRandom;
        *[Self::Sente, Self::Gote].choose(rng).unwrap()
    }

    pub fn is_sente(&self) -> bool {
        matches!(self, Self::Sente)
    }

    pub fn is_gote(&self) -> bool {
        matches!(self, Self::Gote)
    }

    /// [Sente, Gote] のイテレータを返す。
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::Sente, Self::Gote].iter().copied()
    }

    pub fn inv(&self) -> Self {
        match self {
            Self::Sente => Self::Gote,
            Self::Gote => Self::Sente,
        }
    }

    /// 先手なら 1, 後手なら -1 を返す。
    pub fn sgn(&self) -> i32 {
        match self {
            Self::Sente => 1,
            Self::Gote => -1,
        }
    }

    pub fn toggle(&mut self) {
        *self = self.inv();
    }
}

/// 長さ 2 の配列の添字として使えると便利なので
impl<T> std::ops::Index<Side> for [T] {
    type Output = T;

    fn index(&self, side: Side) -> &Self::Output {
        debug_assert_eq!(self.len(), 2);
        &self[side as usize]
    }
}

impl<T> std::ops::IndexMut<Side> for [T] {
    fn index_mut(&mut self, side: Side) -> &mut Self::Output {
        debug_assert_eq!(self.len(), 2);
        &mut self[side as usize]
    }
}

//--------------------------------------------------------------------
// x 座標
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct SqX(i32);

impl SqX {
    pub fn new(value: i32) -> Self {
        Self(value)
    }

    pub fn get(&self) -> i32 {
        self.0
    }

    pub fn is_ok(&self) -> bool {
        (0..11).contains(&self.0)
    }

    pub fn is_valid(&self) -> bool {
        (1..=9).contains(&self.0)
    }

    /// ok な x 座標たちを昇順で返す。
    pub fn iter_ok() -> impl Iterator<Item = Self> {
        (0..11).map(Self)
    }

    /// valid な x 座標たちを昇順で返す。
    pub fn iter_valid() -> impl Iterator<Item = Self> {
        (1..=9).map(Self)
    }

    /// x1, x2 の距離を返す。
    /// x1, x2 のいずれかが ok でない場合、None を返す。
    pub fn dist(x1: Self, x2: Self) -> Option<i32> {
        if !x1.is_ok() || !x2.is_ok() {
            return None;
        }
        Some((x1.0 - x2.0).abs())
    }

    /// 盤面を 180 度回したときの x 座標を返す。
    pub fn inv(&self) -> Self {
        Self(10 - self.0)
    }

    /// side から見たときの x 座標を返す。
    pub fn rel(&self, side: Side) -> Self {
        match side {
            Side::Sente => *self,
            Side::Gote => self.inv(),
        }
    }
}

impl_add_sub!(SqX);

//--------------------------------------------------------------------
// y 座標
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct SqY(i32);

impl SqY {
    pub fn new(value: i32) -> Self {
        Self(value)
    }

    pub fn get(&self) -> i32 {
        self.0
    }

    pub fn is_ok(&self) -> bool {
        (0..11).contains(&self.0)
    }

    pub fn is_valid(&self) -> bool {
        (1..=9).contains(&self.0)
    }

    /// ok な y 座標たちを昇順で返す。
    pub fn iter_ok() -> impl Iterator<Item = Self> {
        (0..11).map(Self)
    }

    /// valid な y 座標たちを昇順で返す。
    pub fn iter_valid() -> impl Iterator<Item = Self> {
        (1..=9).map(Self)
    }

    /// y1, y2 の距離を返す。
    /// y1, y2 のいずれかが ok でない場合、None を返す。
    pub fn dist(y1: Self, y2: Self) -> Option<i32> {
        if !y1.is_ok() || !y2.is_ok() {
            return None;
        }
        Some((y1.0 - y2.0).abs())
    }

    /// 盤面を 180 度回したときの y 座標を返す。
    pub fn inv(&self) -> Self {
        Self(10 - self.0)
    }

    /// side から見たときの y 座標を返す。
    pub fn rel(&self, side: Side) -> Self {
        match side {
            Side::Sente => *self,
            Side::Gote => self.inv(),
        }
    }

    /// side から見て成れる段かどうかを返す。
    pub fn can_promote(&self, side: Side) -> bool {
        (1..=3).contains(&self.rel(side).0)
    }

    /// side から見て pt を配置できる段かどうかを返す。
    /// 行きどころのない駒の判定用。
    pub fn can_put(&self, side: Side, pt: Piece) -> bool {
        let y_rel = self.rel(side).0;
        match pt {
            Piece::Pawn | Piece::Lance => (2..=9).contains(&y_rel),
            Piece::Knight => (3..=9).contains(&y_rel),
            _ => self.is_valid(),
        }
    }
}

impl_add_sub!(SqY);

//--------------------------------------------------------------------
// マス
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Sq(i32);

impl Sq {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn from_xy(x: i32, y: i32) -> Self {
        Self(11 * y + x)
    }

    pub fn get(&self) -> i32 {
        self.0
    }

    pub fn x(&self) -> SqX {
        SqX(self.0 % 11)
    }

    pub fn y(&self) -> SqY {
        SqY(self.0 / 11)
    }

    pub fn xy(&self) -> (SqX, SqY) {
        (self.x(), self.y())
    }

    pub fn is_ok(&self) -> bool {
        (0..11 * 11).contains(&self.0)
    }

    pub fn is_valid(&self) -> bool {
        self.x().is_valid() && self.y().is_valid()
    }

    /// ok なマスたちを昇順で返す。
    pub fn iter_ok() -> impl Iterator<Item = Self> {
        (0..11 * 11).map(Self)
    }

    /// ok なマスたちを降順で返す。
    pub fn iter_ok_rev() -> impl Iterator<Item = Self> {
        (0..11 * 11).rev().map(Self)
    }

    /// valid なマスたちを昇順で返す。
    pub fn iter_valid() -> impl Iterator<Item = Self> {
        Self::iter_ok().filter(Self::is_valid)
    }

    /// valid なマスたちを降順で返す。
    pub fn iter_valid_rev() -> impl Iterator<Item = Self> {
        Self::iter_ok_rev().filter(Self::is_valid)
    }

    /// 原作と同じ順序で valid なマスたちを列挙する。
    /// 原作では盤面が常に your 側から見たものになるため、my が先手のときと後手のときでマスの列挙順
    /// が逆になる。
    pub fn iter_valid_sim(my: Side) -> impl Iterator<Item = Self> {
        match my {
            Side::Sente => Either::Left(Self::iter_valid_rev()),
            Side::Gote => Either::Right(Self::iter_valid()),
        }
    }

    /// いわゆるチェス盤距離を返す。
    /// sq1, sq2 のいずれかが ok でない場合、None を返す。
    pub fn dist(sq1: Self, sq2: Self) -> Option<i32> {
        match (Sq::dist_x(sq1, sq2), Sq::dist_y(sq1, sq2)) {
            (Some(d1), Some(d2)) => Some(std::cmp::max(d1, d2)),
            _ => None,
        }
    }

    /// x 方向の距離を返す。
    /// sq1, sq2 のいずれかが ok でない場合、None を返す。
    pub fn dist_x(sq1: Self, sq2: Self) -> Option<i32> {
        SqX::dist(sq1.x(), sq2.x())
    }

    /// y 方向の距離を返す。
    /// sq1, sq2 のいずれかが ok でない場合、None を返す。
    pub fn dist_y(sq1: Self, sq2: Self) -> Option<i32> {
        SqY::dist(sq1.y(), sq2.y())
    }

    /// 盤面を 180 度回したときのマスを返す。
    pub fn inv(&self) -> Self {
        Self((11 * 11 - 1) - self.0)
    }

    /// side から見たときのマスを返す。
    pub fn rel(&self, side: Side) -> Self {
        match side {
            Side::Sente => *self,
            Side::Gote => self.inv(),
        }
    }

    /// side から見て成れるマスかどうかを返す。
    pub fn can_promote(&self, side: Side) -> bool {
        self.y().can_promote(side)
    }

    /// side から見て pt を配置できるマスかどうかを返す。
    /// 行きどころのない駒の判定用。
    pub fn can_put(&self, side: Side, pt: Piece) -> bool {
        self.y().can_put(side, pt)
    }
}

impl_add_sub!(Sq);

pub const SQ_INVALID: Sq = Sq::new(99);

//--------------------------------------------------------------------
// 駒
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Piece {
    Pawn,
    Lance,
    Knight,
    Silver,
    Bishop,
    Rook,
    Gold,
    King,
    ProPawn,
    ProLance,
    ProKnight,
    ProSilver,
    Horse,
    Dragon,
}

impl Piece {
    /// 持駒となりうるかどうかを返す。
    pub fn is_hand(&self) -> bool {
        matches!(
            self,
            Self::Pawn
                | Self::Lance
                | Self::Knight
                | Self::Silver
                | Self::Bishop
                | Self::Rook
                | Self::Gold
        )
    }

    /// ナマ駒かどうかを返す。
    pub fn is_raw(&self) -> bool {
        !self.is_promoted()
    }

    /// 成駒かどうかを返す。
    pub fn is_promoted(&self) -> bool {
        matches!(
            self,
            Self::ProPawn
                | Self::ProLance
                | Self::ProKnight
                | Self::ProSilver
                | Self::Horse
                | Self::Dragon
        )
    }

    pub fn can_promote(&self) -> bool {
        matches!(
            self,
            Self::Pawn | Self::Lance | Self::Knight | Self::Silver | Self::Bishop | Self::Rook
        )
    }

    /// self が成駒なら対応するナマ駒に変換する。
    /// self がナマ駒なら同じ値を返す。
    pub fn to_raw(&self) -> Self {
        match self {
            Self::ProPawn => Self::Pawn,
            Self::ProLance => Self::Lance,
            Self::ProKnight => Self::Knight,
            Self::ProSilver => Self::Silver,
            Self::Horse => Self::Bishop,
            Self::Dragon => Self::Rook,
            pt => *pt,
        }
    }

    /// self がナマ駒なら対応する成駒に変換する。
    /// self が成駒なら同じ値を返す。
    /// self が成れない駒の場合、None を返す。
    pub fn to_promoted(&self) -> Option<Self> {
        match self {
            Self::Pawn => Some(Self::ProPawn),
            Self::Lance => Some(Self::ProLance),
            Self::Knight => Some(Self::ProKnight),
            Self::Silver => Some(Self::ProSilver),
            Self::Bishop => Some(Self::Horse),
            Self::Rook => Some(Self::Dragon),
            Self::Gold | Self::King => None,
            pt => Some(*pt),
        }
    }

    /// 持駒となりうる駒たちを昇順で返す。
    pub fn iter_hand() -> impl Iterator<Item = Self> {
        [
            Self::Pawn,
            Self::Lance,
            Self::Knight,
            Self::Silver,
            Self::Bishop,
            Self::Rook,
            Self::Gold,
        ]
        .iter()
        .copied()
    }

    /// 近接利き(その方向に1回だけ進める相対インデックスたち)を返す。
    pub fn effects_melee(&self, side: Side) -> impl Iterator<Item = i32> {
        // 後手用の利きは符号反転により計算可能だが、map() などを使うと戻り型が統一できずエラーにな
        // るので、全てベタ書きする。
        let effects: &[i32] = match (side, self) {
            (Side::Sente, Piece::Pawn) => &[-11],
            (Side::Sente, Piece::Knight) => &[-23, -21],
            (Side::Sente, Piece::Silver) => &[-12, -11, -10, 10, 12],
            (Side::Sente, Piece::Gold)
            | (Side::Sente, Piece::ProPawn)
            | (Side::Sente, Piece::ProLance)
            | (Side::Sente, Piece::ProKnight)
            | (Side::Sente, Piece::ProSilver) => &[-12, -11, -10, -1, 1, 11],

            (Side::Gote, Piece::Pawn) => &[11],
            (Side::Gote, Piece::Knight) => &[21, 23],
            (Side::Gote, Piece::Silver) => &[-12, -10, 10, 11, 12],
            (Side::Gote, Piece::Gold)
            | (Side::Gote, Piece::ProPawn)
            | (Side::Gote, Piece::ProLance)
            | (Side::Gote, Piece::ProKnight)
            | (Side::Gote, Piece::ProSilver) => &[-11, -1, 1, 10, 11, 12],

            (_, Piece::King) => &[-12, -11, -10, -1, 1, 10, 11, 12],
            (_, Piece::Horse) => &[-11, -1, 1, 11],
            (_, Piece::Dragon) => &[-12, -10, 10, 12],

            (_, Piece::Lance) | (_, Piece::Bishop) | (_, Piece::Rook) => &[],
        };
        effects.iter().copied()
    }

    /// 遠隔利き(その方向に何回でも進める相対インデックスたち)を返す。
    pub fn effects_ranged(&self, side: Side) -> impl Iterator<Item = i32> {
        let effects: &[i32] = match (side, self) {
            (Side::Sente, Piece::Lance) => &[-11],
            (Side::Gote, Piece::Lance) => &[11],

            (_, Piece::Bishop) | (_, Piece::Horse) => &[-12, -10, 10, 12],
            (_, Piece::Rook) | (_, Piece::Dragon) => &[-11, -1, 1, 11],

            (_, Piece::Pawn)
            | (_, Piece::Knight)
            | (_, Piece::Silver)
            | (_, Piece::Gold)
            | (_, Piece::King)
            | (_, Piece::ProPawn)
            | (_, Piece::ProLance)
            | (_, Piece::ProKnight)
            | (_, Piece::ProSilver) => &[],
        };
        effects.iter().copied()
    }

    /// 原作における駒種 ID を返す。
    pub fn id_naitou(&self) -> i32 {
        match self {
            Piece::King => 1,
            Piece::Rook => 2,
            Piece::Bishop => 3,
            Piece::Gold => 4,
            Piece::Silver => 5,
            Piece::Knight => 6,
            Piece::Lance => 7,
            Piece::Pawn => 8,
            Piece::Dragon => 9,
            Piece::Horse => 10,
            Piece::ProSilver => 12,
            Piece::ProKnight => 13,
            Piece::ProLance => 14,
            Piece::ProPawn => 15,
        }
    }
}

//--------------------------------------------------------------------
// 指し手
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MoveNondrop {
    src: Sq,
    dst: Sq,
    is_promotion: bool,
}

impl MoveNondrop {
    pub fn new(src: Sq, dst: Sq, is_promotion: bool) -> Self {
        assert!(src.is_valid());
        assert!(dst.is_valid());
        assert_ne!(src, dst);

        Self {
            src,
            dst,
            is_promotion,
        }
    }

    pub fn src(&self) -> Sq {
        self.src
    }

    pub fn dst(&self) -> Sq {
        self.dst
    }

    pub fn is_promotion(&self) -> bool {
        self.is_promotion
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MoveDrop {
    pt: Piece,
    dst: Sq,
}

impl MoveDrop {
    pub fn new(pt: Piece, dst: Sq) -> Self {
        assert!(pt.is_hand());
        assert!(dst.is_valid());

        Self { pt, dst }
    }

    pub fn pt(&self) -> Piece {
        self.pt
    }

    pub fn dst(&self) -> Sq {
        self.dst
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Move {
    Nondrop(MoveNondrop),
    Drop(MoveDrop),
}

impl Move {
    pub fn nondrop(src: Sq, dst: Sq, is_promotion: bool) -> Self {
        Self::Nondrop(MoveNondrop::new(src, dst, is_promotion))
    }

    pub fn drop(pt: Piece, dst: Sq) -> Self {
        Self::Drop(MoveDrop::new(pt, dst))
    }

    pub fn from_sfen(sfen: impl AsRef<str>) -> Result<Self> {
        sfen::sfen_to_move(sfen)
    }

    pub fn is_nondrop(&self) -> bool {
        matches!(self, Self::Nondrop(_))
    }

    pub fn is_drop(&self) -> bool {
        matches!(self, Self::Drop(_))
    }

    pub fn is_drop_pt(&self, pt: Piece) -> bool {
        match self {
            Self::Drop(drop) => drop.pt == pt,
            _ => false,
        }
    }

    pub fn dst(&self) -> Sq {
        match self {
            Self::Nondrop(nondrop) => nondrop.dst(),
            Self::Drop(drop) => drop.dst(),
        }
    }

    pub fn is_promotion(&self) -> bool {
        match self {
            Self::Nondrop(nondrop) => nondrop.is_promotion(),
            Self::Drop(_) => false,
        }
    }
}

//--------------------------------------------------------------------
// 盤面
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BoardCell {
    Empty,
    Sente(Piece),
    Gote(Piece),
    Wall,
}

impl BoardCell {
    pub fn from_side_pt(side: Side, pt: Piece) -> Self {
        match side {
            Side::Sente => Self::Sente(pt),
            Side::Gote => Self::Gote(pt),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn is_sente(&self) -> bool {
        matches!(self, Self::Sente(_))
    }

    pub fn is_gote(&self) -> bool {
        matches!(self, Self::Gote(_))
    }

    pub fn is_side(&self, side: Side) -> bool {
        match side {
            Side::Sente => self.is_sente(),
            Side::Gote => self.is_gote(),
        }
    }

    pub fn is_piece(&self) -> bool {
        self.is_sente() || self.is_gote()
    }

    pub fn is_wall(&self) -> bool {
        matches!(self, Self::Wall)
    }

    pub fn piece(&self) -> Option<Piece> {
        match self {
            Self::Sente(pt) => Some(*pt),
            Self::Gote(pt) => Some(*pt),
            _ => None,
        }
    }

    pub fn piece_of(&self, side: Side) -> Option<Piece> {
        match (self, side) {
            (Self::Sente(pt), Side::Sente) => Some(*pt),
            (Self::Gote(pt), Side::Gote) => Some(*pt),
            _ => None,
        }
    }

    pub fn is_side_pt(&self, side: Side, pt_query: Piece) -> bool {
        match self {
            Self::Sente(pt) => side.is_sente() && *pt == pt_query,
            Self::Gote(pt) => side.is_gote() && *pt == pt_query,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Board {
    cells: [BoardCell; 11 * 11],
}

impl Board {
    pub fn empty() -> Self {
        let cells = array_init::from_iter(Sq::iter_ok().map(|sq| {
            if sq.is_valid() {
                BoardCell::Empty
            } else {
                BoardCell::Wall
            }
        }))
        .unwrap();

        Self { cells }
    }

    pub fn row(&self, y: i32) -> &[BoardCell] {
        let y = y as usize;
        &self.cells[11 * y..11 * (y + 1)]
    }

    pub fn row_mut(&mut self, y: i32) -> &mut [BoardCell] {
        let y = y as usize;
        &mut self.cells[11 * y..11 * (y + 1)]
    }

    pub fn row_valid(&self, y: i32) -> &[BoardCell] {
        let y = y as usize;
        &self.cells[11 * y + 1..=11 * y + 9]
    }

    pub fn row_valid_mut(&mut self, y: i32) -> &mut [BoardCell] {
        let y = y as usize;
        &mut self.cells[11 * y + 1..=11 * y + 9]
    }
}

impl std::ops::Index<Sq> for Board {
    type Output = BoardCell;

    fn index(&self, sq: Sq) -> &Self::Output {
        &self.cells[sq.get() as usize]
    }
}

impl std::ops::IndexMut<Sq> for Board {
    fn index_mut(&mut self, sq: Sq) -> &mut Self::Output {
        &mut self.cells[sq.get() as usize]
    }
}

//--------------------------------------------------------------------
// 持駒
//--------------------------------------------------------------------

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Hand([u8; 7]);

impl Hand {
    pub fn empty() -> Self {
        Self([0; 7])
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&count| count == 0)
    }
}

impl std::ops::Index<Piece> for Hand {
    type Output = u8;

    fn index(&self, pt: Piece) -> &Self::Output {
        assert!(pt.is_hand());
        &self.0[pt as usize]
    }
}

impl std::ops::IndexMut<Piece> for Hand {
    fn index_mut(&mut self, pt: Piece) -> &mut Self::Output {
        assert!(pt.is_hand());
        &mut self.0[pt as usize]
    }
}

/// 先手と後手の持駒を束ねたもの。
/// hands[side][pt] のようにアクセスする。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Hands([Hand; 2]);

impl Hands {
    pub fn empty() -> Self {
        Self([Hand::empty(), Hand::empty()])
    }

    pub fn new(hand_sente: Hand, hand_gote: Hand) -> Self {
        Self([hand_sente, hand_gote])
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|hand| hand.is_empty())
    }
}

impl std::ops::Index<Side> for Hands {
    type Output = Hand;

    fn index(&self, side: Side) -> &Self::Output {
        &self.0[side]
    }
}

impl std::ops::IndexMut<Side> for Hands {
    fn index_mut(&mut self, side: Side) -> &mut Self::Output {
        &mut self.0[side]
    }
}

//--------------------------------------------------------------------
// 手合割
//--------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, strum_macros::Display, strum_macros::EnumString)]
pub enum Handicap {
    YourSente,
    YourHishaochi,
    YourNimaiochi,
    MySente,
    MyHishaochi,
    MyNimaiochi,
}

impl Handicap {
    pub fn my(&self) -> Side {
        match self {
            Self::YourSente => Side::Gote,
            Self::YourHishaochi => Side::Gote,
            Self::YourNimaiochi => Side::Gote,
            Self::MySente => Side::Sente,
            Self::MyHishaochi => Side::Sente,
            Self::MyNimaiochi => Side::Sente,
        }
    }

    pub fn your(&self) -> Side {
        self.my().inv()
    }

    pub fn initial_pos(&self) -> Position {
        let pos = |sfen: &str| Position::from_sfen(sfen).unwrap();
        match self {
            Self::YourSente => pos(sfen::SFEN_HIRATE),
            Self::YourHishaochi => pos(sfen::SFEN_HISHAOCHI),
            Self::YourNimaiochi => pos(sfen::SFEN_NIMAIOCHI),
            Self::MySente => pos(sfen::SFEN_HIRATE),
            Self::MyHishaochi => pos(sfen::SFEN_HISHAOCHI),
            Self::MyNimaiochi => pos(sfen::SFEN_NIMAIOCHI),
        }
    }
}
