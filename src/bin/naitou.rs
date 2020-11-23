use naitou_clone::usi;

fn main() -> eyre::Result<()> {
    if cfg!(debug_assertions) {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    usi::interact()?;

    Ok(())
}
