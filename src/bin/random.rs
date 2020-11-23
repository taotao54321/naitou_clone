//! your_move テスト用

use naitou_clone::usi_random;

fn main() -> eyre::Result<()> {
    if cfg!(debug_assertions) {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    usi_random::interact()?;

    Ok(())
}
