use std::path::PathBuf;

use eyre::eyre;
use structopt::StructOpt;

use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;

use naitou_clone::emu::{
    self, Buttons, Cursor, Traveller, BTNS_A, BTNS_D, BTNS_NONE, BTNS_S, BTNS_T, TRAVELLER,
};
use naitou_clone::log::{Logger, LoggerTrait};
use naitou_clone::prelude::*;
use naitou_clone::record::{Record, RecordEntry};

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    path_rom: PathBuf,

    #[structopt(parse(from_os_str))]
    path_record: PathBuf,
}

struct RenderCtx<'a> {
    canvas: Canvas<Window>,
    tex: Texture<'a>,
}

impl<'a> RenderCtx<'a> {
    fn new(canvas: Canvas<Window>, tex: Texture<'a>) -> eyre::Result<Self> {
        Ok(Self { canvas, tex })
    }
}

fn run_frame_hooked(
    ren: &mut RenderCtx,
    buttons: Buttons,
    hook: &dyn FnMut(u16),
) -> eyre::Result<()> {
    let canvas = &mut ren.canvas;
    let tex = &mut ren.tex;

    tex.with_lock(None, |buf, pitch| {
        fceux::run_frame(
            buttons.value(),
            0,
            |xbuf, _| {
                for y in 0..240 {
                    for x in 0..256 {
                        let idx = pitch * y + 4 * x;
                        let (r, g, b) = fceux::video_get_palette(xbuf[256 * y + x]);
                        buf[idx] = 0x00;
                        buf[idx + 1] = b;
                        buf[idx + 2] = g;
                        buf[idx + 3] = r;
                    }
                }
            },
            hook,
        );
    })
    .map_err(|s| eyre!(s))?;

    canvas.copy(&tex, None, None).map_err(|s| eyre!(s))?;
    canvas.present();

    Ok(())
}

#[allow(dead_code)]
fn run_frames_hooked(
    n: i32,
    ren: &mut RenderCtx,
    buttons: Buttons,
    hook: &dyn FnMut(u16),
) -> eyre::Result<()> {
    for _ in 0..n {
        run_frame_hooked(ren, buttons, hook)?;
    }

    Ok(())
}

fn run_frame(ren: &mut RenderCtx, buttons: Buttons) -> eyre::Result<()> {
    run_frame_hooked(ren, buttons, &|_| {})
}

fn run_frames(n: i32, ren: &mut RenderCtx, buttons: Buttons) -> eyre::Result<()> {
    for _ in 0..n {
        run_frame(ren, buttons)?;
    }

    Ok(())
}

fn start_game(ren: &mut RenderCtx, record: &Record) -> eyre::Result<()> {
    let handicap = record.handicap();
    let timelimit = record.timelimit();

    let select_count = match handicap {
        Handicap::YourSente => 0,
        Handicap::YourHishaochi => 2,
        Handicap::YourNimaiochi => 4,
        Handicap::MySente => 6,
        Handicap::MyHishaochi => 8,
        Handicap::MyNimaiochi => 10,
    } + if timelimit { 1 } else { 0 };

    fceux::power();

    run_frames(10, ren, BTNS_NONE)?;

    for _ in 0..select_count {
        run_frame(ren, BTNS_S)?;
        run_frame(ren, BTNS_NONE)?;
    }

    run_frame(ren, BTNS_T)?;

    Ok(())
}

fn wait_your_turn(ren: &mut RenderCtx) -> eyre::Result<()> {
    let mut your_turn = false;
    while !your_turn {
        run_frame_hooked(ren, BTNS_NONE, &|addr| {
            if addr == emu::ADDR_YOUR_TURN {
                your_turn = true;
            }
        })?;
    }

    Ok(())
}

fn move_cursor(ren: &mut RenderCtx, src: &Cursor, dst: &Cursor, interval: i32) -> eyre::Result<()> {
    let i = Traveller::vertex_cursor(src);
    let j = Traveller::vertex_cursor(dst);
    let seq = TRAVELLER.query(i, j);
    for &btns in seq {
        run_frames(3, ren, btns)?;
        run_frames(interval, ren, BTNS_NONE)?;
    }

    Ok(())
}

fn move_your(ren: &mut RenderCtx, mv: &Move, your: Side) -> eyre::Result<()> {
    let promotable = match mv {
        Move::Nondrop(nondrop) => {
            let pt = emu::get_board()[nondrop.src()].piece_of(your).unwrap();
            can_promote(your, pt, nondrop.src(), nondrop.dst())
        }
        Move::Drop(_) => false,
    };

    let src = match mv {
        Move::Nondrop(nondrop) => Cursor::board(nondrop.src().rel(your)),
        Move::Drop(drop) => Cursor::hand(drop.pt()),
    };
    let dst = Cursor::board(mv.dst().rel(your));

    move_cursor(ren, &emu::get_cursor(), &src, 3)?;

    run_frames(3, ren, BTNS_A)?;
    run_frames(4, ren, BTNS_NONE)?;

    move_cursor(ren, &src, &dst, 3)?;

    run_frames(3, ren, BTNS_A)?;

    if promotable {
        run_frames(3, ren, BTNS_NONE)?;
        if !mv.is_promotion() {
            run_frames(3, ren, BTNS_D)?;
            run_frames(3, ren, BTNS_NONE)?;
        }
        run_frames(3, ren, BTNS_A)?;
    }

    Ok(())
}

fn play_my(ren: &mut RenderCtx, _entry: &RecordEntry) -> eyre::Result<()> {
    let mut logger = Logger::new();
    let mut break_flag = false;

    while !break_flag {
        run_frame_hooked(ren, BTNS_NONE, &|addr| match addr {
            emu::ADDR_YOUR_TURN => {
                break_flag = true;
            }
            emu::ADDR_THINK => {
                logger.log_progress(
                    emu::get_progress_ply(),
                    emu::get_progress_level(),
                    emu::get_progress_level_sub(),
                );
                logger.log_book_state(emu::get_book_state());
                logger.log_root_eff_board(emu::get_effect_board());
            }
            emu::ADDR_ROOT_EVALED => {
                logger.log_root_eval(emu::get_root_eval());
                logger.log_best_eval(emu::get_best_eval()); // デフォルト値
            }
            emu::ADDR_TRY_IMPROVE_BEST => {
                logger.start_cand(emu::get_cand_move());
                logger.log_cand_eff_board(emu::get_effect_board());
                logger.log_cand_pos_eval(emu::get_position_eval());
                logger.log_cand_eval(emu::get_cand_eval());
            }
            emu::ADDR_IMPROVE_BEST => {
                logger.log_cand_improve();
            }
            emu::ADDR_TRY_IMPROVE_BEST_DONE_NONDROP | emu::ADDR_TRY_IMPROVE_BEST_DONE_DROP => {
                logger.end_cand();
            }
            emu::ADDR_THINK_DONE => {
                logger.log_best_eval(emu::get_best_eval());
            }
            emu::ADDR_YOUR_SUICIDE => {
                logger.log_record_entry(RecordEntry::YourSuicide);
                break_flag = true;
            }
            emu::ADDR_YOUR_WIN => {
                logger.log_record_entry(RecordEntry::YourWin);
                break_flag = true;
            }
            emu::ADDR_MOVE_MY => {
                logger.log_record_entry(RecordEntry::Move(emu::get_my_move()));
            }
            emu::ADDR_MY_WIN => {
                logger.log_record_entry(RecordEntry::MyWin(emu::get_my_move()));
                break_flag = true;
            }
            addr if emu::ADDRS_TWEAK.contains(&addr) => {
                logger.log_cand_eval(emu::get_cand_eval());
            }
            _ => {}
        })?;
    }

    run_frames(3, ren, BTNS_NONE)?;

    println!("{}", logger.into_log().pretty());

    Ok(())
}

fn play_your(ren: &mut RenderCtx, entry: &RecordEntry, your: Side) -> eyre::Result<()> {
    let mv = match entry {
        RecordEntry::Move(mv) => mv,
        _ => panic!("unexpected your entry: {:?}", entry),
    };

    move_your(ren, mv, your)?;

    Ok(())
}

fn play(ren: &mut RenderCtx, record: &Record) -> eyre::Result<()> {
    let my = record.handicap().my();
    let your = my.inv();

    start_game(ren, record)?;
    if my.is_gote() {
        wait_your_turn(ren)?;
    }

    for (i, entry) in record.entrys().iter().enumerate() {
        let my_turn = match my {
            Side::Sente => i % 2 == 0,
            Side::Gote => i % 2 != 0,
        };

        if my_turn {
            play_my(ren, entry)?;
        } else {
            play_your(ren, entry, your)?;
        }
    }

    Ok(())
}

fn main() -> eyre::Result<()> {
    if cfg!(debug_assertions) {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let opt = Opt::from_args();

    fceux::init(opt.path_rom)?;
    let record = Record::from_file(opt.path_record)?;

    let sdl = sdl2::init().map_err(|s| eyre!(s))?;
    let sdl_video = sdl.video().map_err(|s| eyre!(s))?;

    let win = sdl_video.window("play_record", 512, 480).build()?;
    let canvas = win.into_canvas().build()?;
    let tex_creator = canvas.texture_creator();
    let tex = tex_creator.create_texture_streaming(PixelFormatEnum::RGBX8888, 256, 240)?;
    let mut ren = RenderCtx::new(canvas, tex)?;

    play(&mut ren, &record)?;

    Ok(())
}
