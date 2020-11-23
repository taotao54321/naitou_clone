use naitou_clone::effect::EffectBoard;
use naitou_clone::position::PawnMask;
use naitou_clone::prelude::*;

fn main() -> eyre::Result<()> {
    let mut rng = rand::thread_rng();
    let pos = Position::random(&mut rng);

    println!("{}", pos.pretty());

    let eff_board = EffectBoard::from_board(pos.board(), Side::Sente);
    println!("{}", eff_board.pretty());

    for side in Side::iter() {
        println!("{}", PawnMask::from_board_side(pos.board(), side).pretty());
    }

    Ok(())
}
