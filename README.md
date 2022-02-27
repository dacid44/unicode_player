# unicode_player
after cloning, and with Rust/Cargo installed, run `cargo build --release` and find the executable at `target/unicode_player`, or run `cargo run --release -- <options>` to build and run directly.

You will also need `ffmpeg` installed.

If it stutters, try lowering the framerate with `-f/--framerate <FRAMERATE>`. Most simple videos will run fine at 30fps or their native framerate, but some may need to be lowered. 15fps tends to work pretty well.
