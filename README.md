# Little Falls

A graphics, CPU and memory benchmark for Linux gaming systems, set at a
mossy waterfall in a Northern California forest.

The graphics test is a scripted flythrough of a creek dropping over three
rock ledges: running water, spray and mist, volumetric light shafts through
the canopy, ambient occlusion, backlit maple leaves, ferns, flocking birds
and butterflies. Partway through, the water switches to a long exposure
look, like a slow shutter photo.

Built in Rust on [Bevy](https://bevyengine.org) and Vulkan. Every model and
texture is generated in code, so the repository holds no art assets and
every machine renders exactly the same scene.

> **Status:** early development. Scores are provisional until the
> reference machine is calibrated (see [Scoring](#scoring)).

## Supported systems

Built for gaming-focused Linux distros, including handhelds:

- SteamOS (Steam Deck, ROG Ally / Ally X)
- Bazzite
- Nobara, CachyOS, ChimeraOS
- Any modern x86_64 distro with a Vulkan driver (Mesa RADV, ANV, NVIDIA)

## Running it

Download the AppImage from [Releases](../../releases), then:

```sh
chmod +x LittleFalls-*.AppImage
./LittleFalls-*.AppImage
```

On SteamOS or Bazzite game mode, add the AppImage to Steam as a non-Steam
game and launch it from there.

A full run takes about two minutes: CPU and memory tests first (console),
then the graphics flythrough. Results are saved to
`~/.local/share/little-falls/results/`.

### Options

```
--preset <low|medium|high|ultra>  Graphics quality (default: high)
--duration <seconds>              Length of the recorded flythrough (default: 60)
--windowed <WIDTHxHEIGHT>         Run in a window instead of fullscreen
--cpu-only                        Only run the CPU and memory tests
--graphics-only                   Skip the CPU and memory tests
--explore                         Free-fly camera, no benchmark
--name <nickname>                 Name shown on the scoreboard
--exit-on-finish                  Quit as soon as results are saved
```

Controls: `L` (gamepad Y) toggles long exposure water, `Esc` (gamepad B)
quits.

## What it measures

| Category | Tests |
| --- | --- |
| Graphics | Average FPS and 1% low FPS over the flythrough |
| CPU | Prime sieve, n-body physics, matrix multiply, SHA-256; each single-threaded and on all threads |
| Memory | Read, write and copy bandwidth; random access latency |

Shader compilation hitches are excluded by a warm-up period before timing
starts.

## Scoring

Each test is compared with a reference machine: **Intel Core i9-14900K,
DDR5-6000, AMD Radeon RX 9070 XT, `high` preset at 2560x1440.** That
system scores about **1000** in every category. A system twice as fast
scores 2000; half as fast, 500.

Category scores are geometric means of their tests, and the overall score
is a weighted geometric mean: graphics 50%, CPU 30%, memory 20%.

Graphics scores are only comparable at the same preset and resolution.
The scoreboard groups them accordingly.

The current reference values are estimates. Once the benchmark has run on
the reference hardware they will be locked in and `score_version` bumped.

## Scoreboard

Coming soon: submit your results file through a pull request and a GitHub
Action validates it and adds it to the leaderboard on GitHub Pages. The
results format is documented in [docs/RESULTS_FORMAT.md](docs/RESULTS_FORMAT.md).

## Building from source

```sh
# Debian/Ubuntu build dependencies
sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
cargo build --release
./target/release/little-falls
```

## Project layout

```
crates/lf-core       CPU and memory tests, scoring, results file (no dependencies)
crates/little-falls  The Bevy app: scene, camera flythrough, benchmark runner
packaging/appimage   AppImage packaging
```

## License

GPL-3.0. See [LICENSE](LICENSE).
