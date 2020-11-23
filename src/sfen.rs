//!===================================================================
//! SFEN
//!
//! SFEN の定義は「USI プロトコルの position コマンドの引数として与えられるもの」とする。
//! つまり moves にも対応している。
//!===================================================================

use std::borrow::Cow;

use itertools::{iproduct, Itertools};

use crate::prelude::*;
use crate::{Error, Result};

pub const SFEN_HIRATE: &str =
    "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
pub const SFEN_HISHAOCHI: &str =
    "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B7/LNSGKGSNL b - 1";
pub const SFEN_NIMAIOCHI: &str = "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/9/LNSGKGSNL b - 1";

/// SFEN 文字列をパースし、棋譜 (開始局面, 指し手リスト) を返す。
pub fn sfen_to_kifu(sfen: impl AsRef<str>) -> Result<(Position, Vec<Move>)> {
    let sfen = sfen.as_ref();

    let mut it = sfen.split_ascii_whitespace();
    let mut next = || it.next().ok_or_else(|| Error::invalid_sfen("incomplete"));

    let magic = next()?;
    let pos = match magic {
        "startpos" => sfen_to_position(SFEN_HIRATE),
        "sfen" => {
            let sfen_board = next()?;
            let sfen_side = next()?;
            let sfen_hands = next()?;
            let sfen_ply = next()?;
            let sfen_pos = [magic, sfen_board, sfen_side, sfen_hands, sfen_ply].join(" ");
            sfen_to_position(sfen_pos)
        }
        _ => Err(Error::invalid_sfen(format!(
            "invalid sfen magic: {}",
            magic
        ))),
    }?;

    let mvs = if let Some(magic_moves) = it.next() {
        let sfen_moves = [magic_moves, &it.join(" ")].join(" ");
        sfen_to_moves(sfen_moves)?
    } else {
        Vec::new()
    };

    Ok((pos, mvs))
}

pub fn sfen_to_position(sfen: impl AsRef<str>) -> Result<Position> {
    let sfen = sfen.as_ref();

    let mut it = sfen.split_ascii_whitespace();
    let mut next = || it.next().ok_or_else(|| Error::invalid_sfen("incomplete"));

    let magic = next()?;
    match magic {
        "startpos" => Ok(sfen_to_position(SFEN_HIRATE)?),
        "sfen" => {
            let sfen_board = next()?;
            let sfen_side = next()?;
            let sfen_hands = next()?;
            let sfen_ply = next()?;

            let board = sfen_to_board(sfen_board)?;
            let side = sfen_to_side(sfen_side)?;
            let hands = sfen_to_hands(sfen_hands)?;
            let ply = sfen_to_ply(sfen_ply)?;

            Ok(Position::new(side, board, hands, ply))
        }
        _ => Err(Error::invalid_sfen(format!(
            "invalid sfen magic: {}",
            magic
        ))),
    }
}

pub fn sfen_to_board(sfen: impl AsRef<str>) -> Result<Board> {
    let sfen = sfen.as_ref();

    let sfen_rows: Vec<_> = sfen.split('/').collect();
    chk!(
        sfen_rows.len() == 9,
        Error::invalid_sfen("board: row_count != 9")
    );

    let mut board = Board::empty();
    for (y, sfen_row) in itertools::zip(1.., sfen_rows) {
        board
            .row_valid_mut(y)
            .copy_from_slice(&sfen_to_board_row(sfen_row)?);
    }

    Ok(board)
}

fn sfen_to_board_row(sfen: impl AsRef<str>) -> Result<Vec<BoardCell>> {
    let sfen = sfen.as_ref();

    struct State {
        row: Vec<BoardCell>,
        is_promote: bool,
    }
    impl State {
        fn new() -> Self {
            Self {
                row: Vec::with_capacity(9),
                is_promote: false,
            }
        }
        fn eat(&mut self, c: char) -> Result<()> {
            match c {
                '+' => {
                    self.ensure_size_ok(1)?;
                    self.ensure_not_promote()?;
                    self.is_promote = true;
                }
                c if c.is_ascii_digit() => {
                    self.ensure_not_promote()?;
                    let n = c.to_digit(10).unwrap() as usize;
                    self.ensure_size_ok(n)?;
                    self.row.extend(itertools::repeat_n(BoardCell::Empty, n));
                }
                c if is_piece_char(c) => {
                    self.ensure_size_ok(1)?;
                    let mut pt = char_to_piece(c);
                    if self.is_promote {
                        pt = pt
                            .to_promoted()
                            .ok_or_else(|| Error::invalid_sfen("board_row: cannot promote"))?;
                        self.is_promote = false;
                    }
                    let cell = if c.is_ascii_uppercase() {
                        BoardCell::Sente(pt)
                    } else if c.is_ascii_lowercase() {
                        BoardCell::Gote(pt)
                    } else {
                        unreachable!()
                    };
                    self.row.push(cell);
                }
                _ => return Err(Error::invalid_sfen("board_row: invalid char")),
            }
            Ok(())
        }
        fn ensure_size_ok(&self, n_add: usize) -> Result<()> {
            chk!(
                self.row.len() + n_add <= 9,
                Error::invalid_sfen("board_row: too long row")
            );
            Ok(())
        }
        fn ensure_not_promote(&self) -> Result<()> {
            chk!(
                !self.is_promote,
                Error::invalid_sfen("board_row: invalid '+'")
            );
            Ok(())
        }
    }

    let mut state = State::new();
    for c in sfen.chars() {
        state.eat(c)?;
    }
    chk!(
        state.row.len() == 9,
        Error::invalid_sfen("board_row: incomplete row")
    );

    Ok(state.row)
}

pub fn sfen_to_side(sfen: impl AsRef<str>) -> Result<Side> {
    let sfen = sfen.as_ref();

    match sfen {
        "b" => Ok(Side::Sente),
        "w" => Ok(Side::Gote),
        _ => Err(Error::invalid_sfen("side: invalid side")),
    }
}

/// 枚数上限チェックは行っていない。
pub fn sfen_to_hands(sfen: impl AsRef<str>) -> Result<Hands> {
    let sfen = sfen.as_ref();

    if sfen == "-" {
        return Ok(Hands::empty());
    }

    struct State {
        hands: Hands,
        count: u8,
    }
    impl State {
        fn new() -> Self {
            Self {
                hands: Hands::empty(),
                count: 0,
            }
        }
        fn eat(&mut self, c: char) -> Result<()> {
            match c {
                c if c.is_ascii_digit() => {
                    self.count *= 10;
                    self.count += c.to_digit(10).unwrap() as u8;
                }
                c if is_piece_char(c) => {
                    let pt = char_to_piece(c);
                    chk!(pt.is_hand(), Error::invalid_sfen("hands: not hand piece"));
                    if self.count == 0 {
                        self.count = 1;
                    }
                    let side = if c.is_ascii_uppercase() {
                        Side::Sente
                    } else if c.is_ascii_lowercase() {
                        Side::Gote
                    } else {
                        unreachable!()
                    };
                    self.hands[side][pt] += self.count;
                    self.count = 0;
                }
                _ => return Err(Error::invalid_sfen("hands: invalid char")),
            }
            Ok(())
        }
    }

    let mut state = State::new();
    for c in sfen.chars() {
        state.eat(c)?;
    }
    chk!(
        state.count == 0,
        Error::invalid_sfen("hands: redundant trailing count")
    );

    Ok(state.hands)
}

pub fn sfen_to_ply(sfen: impl AsRef<str>) -> Result<i32> {
    let sfen = sfen.as_ref();

    let ply = sfen
        .parse::<i32>()
        .map_err(|_| Error::invalid_sfen("ply: parse error"))?;

    Ok(ply)
}

pub fn sfen_to_moves(sfen: impl AsRef<str>) -> Result<Vec<Move>> {
    let sfen = sfen.as_ref();

    let mut it = sfen.split_ascii_whitespace();
    let mut next = || it.next().ok_or_else(|| Error::invalid_sfen("incomplete"));

    let magic = next()?;
    chk!(magic == "moves", Error::invalid_sfen("\"moves\" expected"));

    it.map(sfen_to_move).collect::<Result<Vec<_>>>()
}

pub fn sfen_to_move(sfen: impl AsRef<str>) -> Result<Move> {
    let sfen = sfen.as_ref();
    let cs: Vec<_> = sfen.chars().collect();
    chk!(
        (4..=5).contains(&cs.len()),
        Error::invalid_sfen(format!("invalid move: {:?}", sfen))
    );
    if cs.len() == 5 {
        chk!(cs[4] == '+', Error::invalid_sfen("expected '+'"));
    }

    if cs[1] == '*' {
        chk!(
            cs[0].is_ascii_uppercase() && is_piece_char(cs[0]),
            Error::invalid_sfen(format!("invalid piece: {}", sfen))
        );
        let pt = char_to_piece(cs[0]);
        let dst = chars_to_sq(cs[2], cs[3])?;
        Ok(Move::drop(pt, dst))
    } else {
        let src = chars_to_sq(cs[0], cs[1])?;
        let dst = chars_to_sq(cs[2], cs[3])?;
        let is_promote = cs.len() == 5;
        Ok(Move::nondrop(src, dst, is_promote))
    }
}

fn is_piece_char(c: char) -> bool {
    matches!(
        c.to_ascii_uppercase(),
        'P' | 'L' | 'N' | 'S' | 'B' | 'R' | 'G' | 'K'
    )
}

fn char_to_piece(c: char) -> Piece {
    match c.to_ascii_uppercase() {
        'P' => Piece::Pawn,
        'L' => Piece::Lance,
        'N' => Piece::Knight,
        'S' => Piece::Silver,
        'B' => Piece::Bishop,
        'R' => Piece::Rook,
        'G' => Piece::Gold,
        'K' => Piece::King,
        _ => unreachable!(),
    }
}

fn chars_to_sq(cx: char, cy: char) -> Result<Sq> {
    chk!(
        ('1'..='9').contains(&cx),
        Error::invalid_sfen(format!("invalid x: {}", cx))
    );
    chk!(
        ('a'..='i').contains(&cy),
        Error::invalid_sfen(format!("invalid y: {:?}", cy))
    );
    let x = 10 - (cx as u8 - b'0');
    let y = cy as u8 - b'a' + 1;
    Ok(Sq::from_xy(x.into(), y.into()))
}

pub fn kifu_to_sfen(pos: &Position, mvs: &[Move]) -> Cow<'static, str> {
    let sfen_pos = position_to_sfen(pos);

    if mvs.is_empty() {
        sfen_pos
    } else {
        [sfen_pos, moves_to_sfen(mvs)].join(" ").into()
    }
}

pub fn position_to_sfen(pos: &Position) -> Cow<'static, str> {
    let sfen_board = board_to_sfen(pos.board());
    let sfen_side = side_to_sfen(pos.side());
    let sfen_hands = hands_to_sfen(pos.hands());
    let sfen_ply = ply_to_sfen(pos.ply());

    ["sfen".into(), sfen_board, sfen_side, sfen_hands, sfen_ply]
        .join(" ")
        .into()
}

pub fn board_to_sfen(board: &Board) -> Cow<'static, str> {
    (1..=9)
        .map(|y| board_row_to_sfen(board.row_valid(y)))
        .join("/")
        .into()
}

fn board_row_to_sfen(row: &[BoardCell]) -> Cow<'static, str> {
    struct State {
        sfen: String,
        n_empty: i32,
        n_processed: i32,
    }
    impl State {
        fn new() -> Self {
            Self {
                sfen: String::new(),
                n_empty: 0,
                n_processed: 0,
            }
        }
        fn eat(&mut self, cell: &BoardCell) {
            match cell {
                BoardCell::Empty => {
                    self.n_empty += 1;
                    self.n_processed += 1;
                    if self.n_processed == 9 {
                        self.flush_emptys();
                    }
                }
                BoardCell::Sente(pt) => {
                    self.flush_emptys();
                    self.sfen.push_str(&piece_to_sfen(*pt));
                    self.n_processed += 1;
                }
                BoardCell::Gote(pt) => {
                    self.flush_emptys();
                    self.sfen.push_str(&piece_to_sfen(*pt).to_ascii_lowercase());
                    self.n_processed += 1;
                }
                BoardCell::Wall => unreachable!(),
            }
        }
        fn flush_emptys(&mut self) {
            if self.n_empty > 0 {
                self.sfen.push_str(&self.n_empty.to_string());
                self.n_empty = 0;
            }
        }
    }

    let mut state = State::new();
    for cell in row {
        state.eat(cell);
    }

    state.sfen.into()
}

pub fn side_to_sfen(side: Side) -> Cow<'static, str> {
    match side {
        Side::Sente => "b".into(),
        Side::Gote => "w".into(),
    }
}

/// R, B, G, S, N, L, P, r, b, g, s, n, l, p の順で出力する。
/// これが一般的らしい
/// (https://ch.nicovideo.jp/kifuwarabe/blomaga/ar795371)
pub fn hands_to_sfen(hands: &Hands) -> Cow<'static, str> {
    const PIECES: &[Piece] = &[
        Piece::Rook,
        Piece::Bishop,
        Piece::Gold,
        Piece::Silver,
        Piece::Knight,
        Piece::Lance,
        Piece::Pawn,
    ];

    if hands.is_empty() {
        return "-".into();
    }

    let mut sfen = String::new();
    for (side, pt) in iproduct!(&[Side::Sente, Side::Gote], PIECES) {
        let n = hands[*side][*pt];
        if n == 0 {
            continue;
        }
        if n >= 2 {
            sfen.push_str(&n.to_string());
        }
        match side {
            Side::Sente => sfen.push_str(&piece_to_sfen(*pt)),
            Side::Gote => sfen.push_str(&piece_to_sfen(*pt).to_ascii_lowercase()),
        }
    }

    sfen.into()
}

pub fn ply_to_sfen(ply: i32) -> Cow<'static, str> {
    ply.to_string().into()
}

pub fn moves_to_sfen(mvs: &[Move]) -> Cow<'static, str> {
    [
        "moves".into(),
        mvs.iter().map(|mv| move_to_sfen(mv)).join(" "),
    ]
    .join(" ")
    .into()
}

pub fn move_to_sfen(mv: &Move) -> Cow<'static, str> {
    match mv {
        Move::Nondrop(nondrop) => format!(
            "{}{}{}",
            sq_to_sfen(nondrop.src),
            sq_to_sfen(nondrop.dst),
            if nondrop.is_promotion { "+" } else { "" }
        )
        .into(),
        Move::Drop(drop) => format!("{}*{}", piece_to_sfen(drop.pt), sq_to_sfen(drop.dst)).into(),
    }
}

fn piece_to_sfen(pt: Piece) -> Cow<'static, str> {
    match pt {
        Piece::Pawn => "P".into(),
        Piece::Lance => "L".into(),
        Piece::Knight => "N".into(),
        Piece::Silver => "S".into(),
        Piece::Bishop => "B".into(),
        Piece::Rook => "R".into(),
        Piece::Gold => "G".into(),
        Piece::King => "K".into(),
        Piece::ProPawn => "+P".into(),
        Piece::ProLance => "+L".into(),
        Piece::ProKnight => "+N".into(),
        Piece::ProSilver => "+S".into(),
        Piece::Horse => "+B".into(),
        Piece::Dragon => "+R".into(),
    }
}

fn sq_to_sfen(sq: Sq) -> Cow<'static, str> {
    let cx = char::from(10 - sq.x().get() as u8 + b'0');
    let cy = char::from(sq.y().get() as u8 - 1 + b'a');
    format!("{}{}", cx, cy).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chk(sfen: impl AsRef<str>) {
        let sfen = sfen.as_ref();
        let (pos, mvs) = sfen_to_kifu(sfen).unwrap();
        assert_eq!(sfen, kifu_to_sfen(&pos, &mvs));
    }

    fn chk2(sfen_from: impl AsRef<str>, sfen_to: impl AsRef<str>) {
        let sfen_from = sfen_from.as_ref();
        let sfen_to = sfen_to.as_ref();
        let (pos, mvs) = sfen_to_kifu(sfen_from).unwrap();
        assert_eq!(sfen_to, kifu_to_sfen(&pos, &mvs));
    }

    #[test]
    fn test() {
        chk("sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1");
        chk("sfen 8l/1l+R2P3/p2pBG1pp/kps1p4/Nn1P2G2/P1P1P2PP/1PS6/1KSG3+r1/LN2+p3L w Sbgn3p 1");
        chk("sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1 moves 7g7f 3c3d 8h2b+ 3a2b B*4e B*8e 4e3d 8e7f");

        chk2(
            "startpos",
            "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        );
        chk2(
            "startpos moves 7g7f 3c3d 2g2f",
            "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1 moves 7g7f 3c3d 2g2f",
        );
        chk2("startpos moves 7g7f 3c3d 8h2b+ 3a2b B*4e B*8e 4e3d 8e7f", "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1 moves 7g7f 3c3d 8h2b+ 3a2b B*4e B*8e 4e3d 8e7f");
    }
}
