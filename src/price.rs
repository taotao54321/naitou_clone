//!===================================================================
//! 駒価値テーブル
//!
//! 駒価値テーブルは 4 種類ある。値の根拠はよくわかっていない。
//! ナマ駒の値は全テーブル共通。
//! 一部の成駒はテーブルごとに値が異なる。コメントで (*) が付いているものが該当。
//!===================================================================

use crate::prelude::*;

#[derive(Debug)]
pub struct PriceTable([u8; 14]);

impl PriceTable {
    const fn new(prices: [u8; 14]) -> Self {
        Self(prices)
    }
}

impl std::ops::Index<Piece> for PriceTable {
    type Output = u8;

    fn index(&self, pt: Piece) -> &Self::Output {
        &self.0[pt as usize]
    }
}

/// 駒価値テーブル0
///
/// 用途:
///
///   * attacker 更新時の比較基準
///   * capture_price の値
pub const PRICES_0: PriceTable = PriceTable::new([
    1,  // 歩
    4,  // 香
    4,  // 桂
    8,  // 銀
    16, // 角
    17, // 飛
    8,  // 金
    40, // 玉
    2,  // 成歩 (*)
    5,  // 成香 (*)
    6,  // 成桂 (*)
    8,  // 成銀
    20, // 馬   (*)
    22, // 龍
]);

/// 駒価値テーブル1
///
/// 用途:
///
///   * 駒得マス判定における your 駒、my attacker の価値算定
pub const PRICES_1: PriceTable = PriceTable::new([
    1,  // 歩
    4,  // 香
    4,  // 桂
    8,  // 銀
    16, // 角
    17, // 飛
    8,  // 金
    40, // 玉
    8,  // 成歩 (*)
    8,  // 成香 (*)
    8,  // 成桂 (*)
    8,  // 成銀
    22, // 馬   (*)
    22, // 龍
]);

/// 駒価値テーブル2
///
/// 用途:
///
///   * 駒損マス判定における your attacker の価値算定
pub const PRICES_2: PriceTable = PriceTable::new([
    1,  // 歩
    4,  // 香
    4,  // 桂
    8,  // 銀
    16, // 角
    17, // 飛
    8,  // 金
    40, // 玉
    2,  // 成歩 (*)
    8,  // 成香 (*)
    8,  // 成桂 (*)
    8,  // 成銀
    22, // 馬   (*)
    22, // 龍
]);

/// 駒価値テーブル3
///
///   * 駒損マス判定における my 駒、my attacker の価値算定
pub const PRICES_3: PriceTable = PriceTable::new([
    1,  // 歩
    4,  // 香
    4,  // 桂
    8,  // 銀
    16, // 角
    17, // 飛
    8,  // 金
    40, // 玉
    1,  // 成歩 (*)
    4,  // 成香 (*)
    4,  // 成桂 (*)
    8,  // 成銀
    20, // 馬   (*)
    22, // 龍
]);
