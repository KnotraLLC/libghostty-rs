# ghostty

Safe Rust API over `ghostty-sys`.

Handle types (`Terminal`, `RenderState`, `KeyEncoder`, etc.) are `!Send + !Sync` by design. Callers should drive all operations from a single thread.
