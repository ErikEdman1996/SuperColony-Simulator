# Supercolony — local Rust prototype

A native, real-time ant colony gameplay test. Built with **Rust + Macroquad**.
There is no server, browser, account system, or multiplayer infrastructure.

## Run here (WSL)

```bash
./run.sh
```

The launcher uses installed Rust, or the isolated Rust toolchain prepared under
`/tmp/opencode/` in this environment. That fallback is temporary: if it is removed,
install Rust from <https://rustup.rs>. WSL needs WSLg / an accessible display.

With a normal Rust installation, including on Windows:

```bash
cargo run --release
```

On Linux, Macroquad needs working OpenGL and X11 runtime libraries. Audio is
disabled. This project does not incorporate OpenFront code or assets: its original,
procedural, world-like map takes inspiration from the territorial-map presentation.
It is not an accurate geographic map.

## Play

1. Click a land zone to place your queen. The clock starts on placement.
2. Ringed neighbouring zones are available for expansion. Hover to inspect food.
3. Click one: its overhead progress bar runs for **6 simulation seconds**.
4. An empty zone becomes yours. An occupied zone opens the automatic attack view.
5. Watch the ants fight. Victory captures the target; defeat leaves it with its owner.
6. After battle, return to the world map to select the next zone.

| Control | Action |
| --- | --- |
| Left click | Found colony / choose neighbouring zone |
| Scroll wheel | Zoom world map toward cursor |
| Right drag | Pan world map |
| Home | Reset map camera |
| Tab / view buttons | Switch world and current/latest battle |
| Space | Pause/resume the entire simulation |
| 1x / 2x / 4x buttons | Change simulation speed |
| R / NEW | New world, with the next seed |
| Escape | Quit |

For reproducible initial maps: `./run.sh --seed 1234`.

## Agreed scope

- Two views only: world and automatic combat.
- Player-selected adjacent expansion, one operation at a time, with a progress bar.
- Attack army derived from current colony population.
- Real-time simulation; identical initial colony populations, supplies and ant stats.
- Each zone contains a randomly assigned **finite, non-regenerating** food reserve.
- Food is needed for production and upkeep.
- No evolution cards, abilities, nest view, or player-issued battle commands.

## Explicit provisional rules

These enable testing; they are **not final design decisions**. Edit `tuning.toml`
and restart to change them. Invalid configuration is reported rather than silently
replaced with defaults.

| Parameter | Initial test value |
| --- | --- |
| Starting colonies | Player + four rivals |
| Starting ants, each colony | 1,200 |
| Starting stored food, each colony | 1,500 |
| Food per zone | Seeded random value from 12,000 to 32,000 |
| Expansion duration | 6 seconds; encounter occurs when the timer completes |
| Committed army | Floor of 50% of current population, minimum 1 for a living colony |
| Defender commitment | Same population formula |
| Harvest | 0.09 food / non-deployed ant / second |
| Harvest distribution | Workers divided evenly over owned zones |
| Harvest ceiling | 120 food / zone / second |
| Upkeep | 0.025 food / living ant / second, including deployed ants |
| Production | Up to 2 ants / second / colony, 8 food each |
| Production reserve | Keep 20 seconds of current upkeep before spending on births |
| Starvation | 8 unfed ant-seconds cause one death; fractional deficits carry over |
| Ant stats | Identical health, speed, reach and damage for all colonies |
| World during combat | Economy continues, including when changing views |
| Rival behaviour | Harvest and reproduce; no autonomous expansion yet |

New ants are produced immediately; no brood or maturation simulation is included.
Food stores are pooled per colony, with no logistics/storage limits. A queen is
the founding action, not a separately simulated agent. A depleted zone remains
owned. There is no win condition or water crossing in this test.

Army membership is a **subset** of living colony population. Deployment does not
subtract the army from population or create extra ants. Deployed ants still eat,
but do not harvest. Casualties reduce population once. Starvation first reduces
non-deployed population; if that is exhausted, deployed ants are removed too.
Survivors simply become available again when combat ends. The defender retains
the zone on mutual annihilation. A colony at zero population cannot restart births.
The simulation stops when the player's colony dies.

One nearby rival is placed across a starting border to make combat easy to test;
other rivals are spread out. Random zone resources mean initial *environments*
differ even though colony starting conditions and ant stats are identical.

## Simulation and rendering

- Renderer-independent simulation modules (`world`, `sim`, `battle`).
- Fixed 30 Hz simulation with position interpolation for rendering.
- Seeded map generation, resources, and battle placement.
- Connected grid zones and symmetric land-border adjacency.
- Stock/flow economy with bounded harvesting, explicit consumption and fractional
  birth/starvation accounting. No food regeneration.
- Individual battle agents, grid-based local targeting, local separation and
  simultaneous damage. No general rigid-body physics or all-pairs global search.
- One procedural ant texture, batched sprite drawing; only world ownership changes
  rebuild the map texture. Tiny sinusoidal rotations provide placeholder animation,
  not a fully articulated walking cycle.
- Combat reuses spatial buckets, movement and damage buffers between ticks.
- A maximum of 12 catch-up steps per rendered frame avoids a runaway backlog. If
  the machine cannot keep up, simulation time slows instead of using larger steps.

No specific maximum ant count is promised. Dense local crowds can still make the
spatial search expensive, and this simple steering model is not a final combat AI.

## Tests and performance

```bash
cargo test
cargo run --release --bin bench -- 10000 900
```

The benchmark runs without a window and reports median/p95/p99/max tick time, both
overall and after the first casualty. Arguments are total ants (split between two
teams) and maximum simulation ticks. It measures **combat CPU work only**, not
rendering, interface, economy, or end-to-end FPS. Use release builds for comparisons.
The game footer shows FPS and a smoothed simulation tick measurement.

### Initial measurements in this WSL environment

Release build, seed 42, up to 900 ticks, one run per workload:

| Total battle agents | Median tick | p99 tick | Maximum tick |
| --- | --- | --- | --- |
| 10,000 | 1.37 ms | 2.62 ms | 3.16 ms |
| 50,000 | 29.09 ms | 63.75 ms | 68.46 ms |

These are **headless CPU simulation results**, not FPS claims. The 50,000-agent
test exceeds the 33.33 ms tick budget during dense contact and needs further
optimisation; it is not a supported smooth-play target. The default starts at
1,200 ants per colony, committing roughly 600 per side in an early battle.
Window/screenshot smoke checks passed on WSLg using the D3D12 AMD Radeon RX 6700 XT
renderer. Sustained end-to-end rendering performance has not been benchmarked.

Developer-only screenshot checks (require a display):

```bash
./run.sh --capture world /tmp/opencode/supercolony-world.png --seed 1234
./run.sh --capture battle /tmp/opencode/supercolony-battle.png --seed 1234
```

## Files

- `src/main.rs`: native window, two views, input, procedural rendering.
- `src/world.rs`: original map generator, zones, ownership, adjacency.
- `src/sim.rs`: real-time clock, economy, expansion, population accounting, tests.
- `src/battle.rs`: individual-agent automatic combat.
- `src/config.rs` and `tuning.toml`: explicit, validated test parameters.
- `src/bin/bench.rs`: window-free combat benchmark.
