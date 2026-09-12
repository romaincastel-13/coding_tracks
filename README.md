# Local Train Tracks — CodinGame League 1 replica

This is a small, dependency-free local harness for the train-track challenge.

## Requirements

- Rust stable toolchain
- Cargo

## Build

```bash
cargo build
```

## Run the local five-game test

```bash
cargo run --bin runner
```

The runner starts `target/debug/bot`, sends it the same initialization/turn protocol, and acts as the Boss AI by skipping its turns.

You can also pass another bot executable:

```bash
cargo run --bin runner -- ./path/to/my_bot
```

On Windows:

```powershell
cargo run --bin runner -- .\path\to\my_bot.exe
```

## Bot

`src/bin_bot.rs` is the actual stdin/stdout bot.

Its current League 1 strategy is deliberately simple:

1. Find a unilateral desired connection.
2. If it is not already connected, issue `AUTOPLACE` between the two towns.
3. Otherwise wait.

Replace `choose_action()` with your own strategy.

## Important local-engine assumptions

The supplied challenge rules are enough for League 1, but some CodinGame engine details are not fully specified in the text. This harness therefore makes explicit, replaceable assumptions:

- The Boss skips every turn.
- A turn gives each player 3 paint points.
- `AUTOPLACE` is expanded to the cheapest route measured by the cost of newly painted cells.
- Existing tracks and towns have zero additional paint cost for AUTOPLACE.
- Dijkstra is used for cheapest paths.
- Active connection shortest paths use BFS with neighbor order N, E, S, W.
- Multiple players painting the same cell during the same turn creates neutral owner `2`.
- Neutral tracks score for neither player.
- Invalid placements are ignored locally rather than terminating the process.
- `MESSAGE` is omitted from the local action parser because it has no game-state effect; add it if you want viewer-style logging.
- The generated maps are deterministic and intentionally simple. They are not claimed to reproduce CodinGame's random map generator.

For a submission bot, keep the bot's protocol-facing code compatible with CodinGame and avoid relying on the runner internals.

## Suggested next step

Once this runs, move the reusable game logic into a library and add:

- random legal map generation matching the real constraints
- a stronger opponent
- batch simulation (hundreds/thousands of games)
- a bot-vs-bot mode
- deterministic replay files
- per-turn timing
- action legality diagnostics
- unit tests for path tie-breaking and scoring
