use naitou_clone::emu::{self, Cursor, Traveller, BTNS_NONE, BTNS_T, TRAVELLER};
use naitou_clone::prelude::*;

fn iter_cursors() -> impl Iterator<Item = Cursor> {
    itertools::chain(iter_cursors_board(), iter_cursors_hand())
}

fn iter_cursors_board() -> impl Iterator<Item = Cursor> {
    Sq::iter_valid().map(Cursor::board)
}

fn iter_cursors_hand() -> impl Iterator<Item = Cursor> {
    const PTS: &[Piece] = &[
        Piece::Rook,
        Piece::Bishop,
        Piece::Gold,
        Piece::Silver,
        Piece::Knight,
        Piece::Lance,
        Piece::Pawn,
    ];
    PTS.iter().map(|pt| Cursor::Hand(*pt))
}

fn move_cursor(src: &Cursor, dst: &Cursor) {
    let i = Traveller::vertex_cursor(src);
    let j = Traveller::vertex_cursor(dst);
    let seq = TRAVELLER.query(i, j);
    for &btns in seq {
        emu::run_frame(btns);
        emu::run_frame(BTNS_NONE);
    }
}

fn usage() -> ! {
    eprintln!("Usage: traveller_test <naitou.nes>");
    std::process::exit(1);
}

fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        usage();
    }
    let path_rom = &args[1];

    fceux::init(path_rom)?;

    emu::run_frames(10, BTNS_NONE);
    emu::run_frame(BTNS_T);

    emu::run_frames(100, BTNS_NONE);

    let cursor = emu::get_cursor();
    assert_eq!(cursor, Cursor::board(Sq::from_xy(5, 5)));

    let snap = fceux::snapshot_create();
    fceux::snapshot_save(&snap)?;

    for src in iter_cursors() {
        for dst in iter_cursors() {
            move_cursor(&cursor, &src);
            assert_eq!(emu::get_cursor(), src);
            move_cursor(&src, &dst);
            /*
            if emu::get_cursor() != dst {
                dbg!(&src, &dst);
                dbg!(TRAVELLER.query(
                    Traveller::vertex_cursor(&src),
                    Traveller::vertex_cursor(&dst)
                ));
            }
            */
            assert_eq!(emu::get_cursor(), dst);

            fceux::snapshot_load(&snap)?;
        }
    }

    Ok(())
}
