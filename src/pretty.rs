use std::borrow::Cow;

use itertools::Itertools;

use crate::effect::EffectBoard;
use crate::position::PawnMask;
use crate::prelude::*;

pub trait Pretty {
    fn pretty(&self) -> Cow<'static, str>;
}

impl Pretty for Side {
    fn pretty(&self) -> Cow<'static, str> {
        match self {
            Side::Sente => "先手".into(),
            Side::Gote => "後手".into(),
        }
    }
}

impl Pretty for SqX {
    /// ```
    /// # use naitou_clone::prelude::*;
    /// assert_eq!(SqX::new(1).pretty(), "９");
    /// assert_eq!(SqX::new(9).pretty(), "１");
    /// ```
    fn pretty(&self) -> Cow<'static, str> {
        const STRS: &[&str] = &["１", "２", "３", "４", "５", "６", "７", "８", "９"];

        assert!(self.is_valid());
        STRS[9 - self.0 as usize].into()
    }
}

impl Pretty for SqY {
    /// ```
    /// # use naitou_clone::prelude::*;
    /// assert_eq!(SqY::new(1).pretty(), "一");
    /// assert_eq!(SqY::new(9).pretty(), "九");
    /// ```
    fn pretty(&self) -> Cow<'static, str> {
        const STRS: &[&str] = &["一", "二", "三", "四", "五", "六", "七", "八", "九"];

        assert!(self.is_valid());
        STRS[self.0 as usize - 1].into()
    }
}

impl Pretty for Sq {
    /// ```
    /// # use naitou_clone::prelude::*;
    /// assert_eq!(Sq::from_xy(3, 6).pretty(), "７六");
    /// ```
    fn pretty(&self) -> Cow<'static, str> {
        format!("{}{}", self.x().pretty(), self.y().pretty()).into()
    }
}

impl Pretty for Piece {
    fn pretty(&self) -> Cow<'static, str> {
        match self {
            Piece::Pawn => "歩".into(),
            Piece::Lance => "香".into(),
            Piece::Knight => "桂".into(),
            Piece::Silver => "銀".into(),
            Piece::Bishop => "角".into(),
            Piece::Rook => "飛".into(),
            Piece::Gold => "金".into(),
            Piece::King => "玉".into(),
            Piece::ProPawn => "と".into(),
            Piece::ProLance => "杏".into(),
            Piece::ProKnight => "圭".into(),
            Piece::ProSilver => "全".into(),
            Piece::Horse => "馬".into(),
            Piece::Dragon => "龍".into(),
        }
    }
}

impl Pretty for MoveNondrop {
    fn pretty(&self) -> Cow<'static, str> {
        format!(
            "{}{}{}",
            self.src.pretty(),
            self.dst.pretty(),
            if self.is_promotion { "成" } else { "" }
        )
        .into()
    }
}

impl Pretty for MoveDrop {
    fn pretty(&self) -> Cow<'static, str> {
        format!("{}{}打", self.dst.pretty(), self.pt.pretty()).into()
    }
}

impl Pretty for Move {
    /// ```
    /// # use naitou_clone::prelude::*;
    /// assert_eq!(Move::nondrop(Sq::from_xy(3, 7), Sq::from_xy(3, 6), false).pretty(), "７七７六");
    /// assert_eq!(Move::nondrop(Sq::from_xy(8, 8), Sq::from_xy(8, 3), true).pretty(), "２八２三成");
    /// assert_eq!(Move::drop(Piece::Gold, Sq::from_xy(6, 1)).pretty(), "４一金打");
    /// ```
    fn pretty(&self) -> Cow<'static, str> {
        match self {
            Self::Nondrop(nondrop) => nondrop.pretty(),
            Self::Drop(drop) => drop.pretty(),
        }
    }
}

impl Pretty for BoardCell {
    fn pretty(&self) -> Cow<'static, str> {
        match self {
            Self::Empty => " 口".into(),
            Self::Sente(pt) => format!(" {}", pt.pretty()).into(),
            Self::Gote(pt) => format!("v{}", pt.pretty()).into(),
            Self::Wall => " 壁".into(),
        }
    }
}

impl Pretty for Board {
    fn pretty(&self) -> Cow<'static, str> {
        let mut res = String::new();

        for y in 1..=9 {
            for x in 1..=9 {
                res.push_str(&self[Sq::from_xy(x, y)].pretty());
            }
            res.push('\n');
        }

        res.into()
    }
}

impl Pretty for Hand {
    fn pretty(&self) -> Cow<'static, str> {
        const PIECES: &[Piece] = &[
            Piece::Rook,
            Piece::Bishop,
            Piece::Gold,
            Piece::Silver,
            Piece::Knight,
            Piece::Lance,
            Piece::Pawn,
        ];

        PIECES
            .iter()
            .filter_map(|&pt| {
                let n = self[pt];
                if n == 0 {
                    None
                } else if n == 1 {
                    Some(pt.pretty())
                } else {
                    Some(format!("{}{}", pt.pretty(), n).into())
                }
            })
            .join(" ")
            .into()
    }
}

impl Pretty for Hands {
    fn pretty(&self) -> Cow<'static, str> {
        format!(
            "\
先手持駒:{}
後手持駒:{}
",
            self[Side::Sente].pretty(),
            self[Side::Gote].pretty()
        )
        .into()
    }
}

impl Pretty for Position {
    fn pretty(&self) -> Cow<'static, str> {
        format!(
            "\
手番:{}
後手持駒:{}
{}先手持駒:{}
{}
",
            self.side().pretty(),
            self.hand(Side::Gote).pretty(),
            self.board().pretty(),
            self.hand(Side::Sente).pretty(),
            self.to_sfen()
        )
        .into()
    }
}

impl Pretty for EffectBoard {
    fn pretty(&self) -> Cow<'static, str> {
        let mut res = String::new();

        for side in Side::iter() {
            res.push_str(&format!("{}\n", side.pretty()));

            for y in 0..11 {
                for x in 0..11 {
                    let info = &self[Sq::from_xy(x, y)][side];
                    res.push_str(&format!(
                        "{}{} ",
                        info.count(),
                        info.attacker()
                            .map_or_else(|| "  ".into(), |pt| pt.pretty())
                    ));
                }
                res.push('\n');
            }

            res.push('\n');
        }

        res.into()
    }
}

impl Pretty for PawnMask {
    fn pretty(&self) -> Cow<'static, str> {
        format!("[{}]", (1..=9).filter(|&x| self.test(x)).join(", ")).into()
    }
}
