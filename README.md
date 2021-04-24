# Masof

Experimental helper Rust library for writing console applications.

Guiding principles:

- Double buffer rendering. Important for easy drawing code, minimal
differential terminal traffic and overlaying things on top of each other
without worrying over flickering.
- Support either full screen or extendible bottom screen (like in FZF).
- Key mapping management and mapping to actions.
- Various widgets with own customizable key bindings and appearance.
- Separate themes from widget implementation - color scheme is externally
  brought.

### Very basic demo

```
cargo run --example masof-simple -- -b 10
```
