# Little Falls

A graphics, CPU and memory benchmark for Linux gaming systems, set at a
mossy waterfall in a Northern California forest.

The graphics test is a scripted flythrough of a creek dropping over three
rock ledges: running water, spray and mist, volumetric light shafts through
the canopy, ambient occlusion, backlit maple leaves, ferns, flocking birds
and butterflies. Partway through, the water switches to a long exposure
look, like a slow shutter photo.

Built in Rust on [Bevy](https://bevyengine.org) and Vulkan, with
photo-scanned rocks, plants, wood, ground surfaces and sky lighting from
[Poly Haven](https://polyhaven.com) (CC0).

> **Status:** early development. Scores may change between versions
> while the tests are tuned; results record the scoring version.

## Supported systems

Built for gaming-focused Linux distros, including handhelds:

- SteamOS (Steam Deck, ROG Ally / Ally X)
- Bazzite
- Nobara, CachyOS, ChimeraOS
- Any modern x86_64 distro with a Vulkan driver (Mesa RADV, ANV, NVIDIA)

## Install

**One command, straight from GitHub** (no root needed, works on SteamOS
and Bazzite):

```sh
curl -fsSL https://raw.githubusercontent.com/Ravenor907/Little-Falls-Benchmark/main/install.sh | sh
```

It installs the Flatpak when Flatpak is available, otherwise the
AppImage, and adds Little Falls to your application menu. Run it again to
update; `sh install.sh --uninstall` removes it.

**Or download from [Releases](../../releases):**

- `LittleFalls.flatpak`: `flatpak install --user LittleFalls.flatpak`
- `LittleFalls-*.AppImage`: no install at all, just
  `chmod +x LittleFalls-*.AppImage` and run it
- `little-falls-*.tar.gz`: the plain binary with its assets folder

On SteamOS or Bazzite game mode, add Little Falls to Steam as a non-Steam
game (from desktop mode) and launch it from there.

## Using it

The app opens on the waterfall with a menu:

- **Run benchmark**: CPU and memory tests, then the graphics flythrough
  (about two minutes)
- **Graphics only** / **CPU and memory**: run one part
- **History**: your previous runs
- **Explore**: fly around the scene freely
- **Quality**: Low, Medium, High (the standard scoreboard test) or Ultra

Mouse, keyboard and gamepad all work: arrows or D-pad to move, Enter or A
to select, Esc or B to go back. `L` / Y toggles long exposure water.

Results are shown in the app and saved to
`~/.local/share/little-falls/results/` (Flatpak:
`~/.var/app/io.github.Ravenor907.LittleFalls/data/little-falls/results/`).

### Reliable results

Little Falls checks its own measurements:

- Shaders are compiled before timing starts (the camera pre-flies the
  whole path behind a loading screen), so compile stutter is not counted
- Every CPU and memory test is repeated; large run-to-run variation is
  flagged
- A frame rate pinned to a display refresh rate (vsync or a frame
  limiter such as gamescope's) is detected and flagged
- Flagged runs show a warning on the results screen and in the results
  file

### Command line options

For scripts and automated testing; not needed for normal use.

```
--preset <low|medium|high|ultra>  Graphics quality (default: high)
--run <full|graphics|cpu>         Skip the menu and start a run
--windowed <WIDTHxHEIGHT>         Run in a window instead of fullscreen
--cpu-only                        CPU and memory tests in the terminal, no window
--duration <seconds>              Length of the recorded flythrough (default: 60)
--name <nickname>                 Name shown on the scoreboard
--exit-on-finish                  Quit as soon as results are saved
```

## What it measures

| Category | Tests |
| --- | --- |
| Graphics | Average FPS and 1% low FPS over the flythrough |
| CPU | Prime sieve, n-body physics, matrix multiply, SHA-256; each single-threaded and on all threads |
| Memory | Read, write and copy bandwidth; random access latency |

Shader compilation hitches are excluded by a warm-up period before timing
starts.

## Scoring

Higher is better. Each test converts its measurement to points linearly,
so a system twice as fast scores twice as many points. Category scores
are geometric means of their tests, and the overall score is a weighted
geometric mean: graphics 50%, CPU 30%, memory 20%.

Graphics scores are only comparable at the same preset and resolution.
The scoreboard groups them accordingly.

## Scoreboard

Coming soon: submit your results file through a pull request and a GitHub
Action validates it and adds it to the leaderboard on GitHub Pages. The
results format is documented in [docs/RESULTS_FORMAT.md](docs/RESULTS_FORMAT.md).

## Building from source

```sh
# Debian/Ubuntu build dependencies
sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
python3 tools/fetch_assets.py      # downloads the CC0 art assets (~170 MB)
cargo build --release
./target/release/little-falls
```

Without the assets folder the app still runs, with a simpler fully
procedural scene.

## Project layout

```
crates/lf-core       CPU and memory tests, scoring, results file (no dependencies)
crates/lf-image      Mipmaps, HDR decoding, image based lighting (no dependencies)
crates/little-falls  The Bevy app: scene, shaders, flythrough, interface
tools/               Asset manifest and fetcher
packaging/           AppImage and Flatpak packaging
install.sh           One-line installer
```

## License

GPL-3.0. See [LICENSE](LICENSE).

Art assets are CC0 from [Poly Haven](https://polyhaven.com); see
`assets/CREDITS.txt` after fetching. The embedded [Inter](https://rsms.me/inter/) font is licensed under the SIL Open Font
License 1.1 ([crates/little-falls/assets/fonts/OFL.txt](crates/little-falls/assets/fonts/OFL.txt)).
