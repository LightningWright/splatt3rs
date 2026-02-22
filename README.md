# Splatoon 3 Automatic Mailbox Message Plotter

This project draws the desired pixel art provided by the user into the mailbox message.
It consists in three key components:
- Optimizer: it computes _almost_ the optimal strategy to draw all pixels. It generates an instruction file.
- Simulator: creates a GIF animation given an instruction file.
- Microcontroller implementation: executes the instruction file on a virtual controller.

## Requirements

 - Hardware:
   - Microcontroller: RP2040 (Connects to the Nintendo Switch as a virtual controller and prints)
   - USB Type Micro B to Type A cable
 - Software:
   - Rust Toolchain (only for compiling)

## Quick Guide
## Components
### Painting Algorithm Optimizer

![Image of Mia Fey in 1bpp PNG format](docs/input.png)

### Simulator

The simulator creates a GIF animation given an instructions file.
It can be run with the following command:

`cargo r --release -p simulator -- -v -d -f 16`

![Simulation of the input image](docs/output.gif)

All the options that are available can be checked by passing "-h" flag:
- -i, --input <INPUT>          [default: instrs.rle]
- -o, --output <OUTPUT>        [default: output.gif/png]
- -v, --video Output a GIF animation
- -d, --video-debug Trace the cursor movement in cyan.
- -I, --image. Outputs only the final result
- -f, --framestep <FRAMESTEP>  [default: 1] Skip N frames
- -h, --help                   Print help
- -V, --version                Print version

### Microcontroller implementation

TODO

Toolchain for ARMv6-M (RP2040 Cortex-M0), stack protection and flashing tools.
   ```
   rustup target install thumbv6m-none-eabi
   cargo install flip-link
   cargo install --locked probe-rs-tools
   ```
