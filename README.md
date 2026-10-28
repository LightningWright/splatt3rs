# Splatoon 3 Automatic Mailbox Message Plotter

This project draws automatically any given pixel art into the mailbox message.
It consists in three key components:
- Optimizer: it computes _almost_ the optimal strategy to draw all pixels. It generates a compressed list of instructions (button presses): instrs.rle.
- Simulator: creates a GIF animation given an instruction list. (For debug purposes)
- Microcontroller implementation: executes the instruction file on a virtual controller.

## Requirements

 - Hardware:
   - Microcontroller: RP2040 (Any of the variants) (Connects to the Nintendo Switch as a virtual controller and prints)
   - USB Type Micro B to Type A / Type C to Type C cable (Depending on the variant)
 - Software:
   - Rust Toolchain (only for compiling)
   - Python 3 Interpreter

## Quick Guide

- Obtain or draw a 320x120 picture in 1BPP PNG format. (See input/*.png files for reference)
- Compile and run the optimizer. An instruction list file is created.
- Compile (optional) and patch the embedded binary with the instruction list.
- Flash the patched binary into your RP2040 microcontroller.
    - If you have a flasher RP-Pico or a generic probe, refer to [Debug Probe](rp2040-impl/debug_probes.md).
    - If you don't have a probe, use the elf2uf2 tool and drag and drop the binary when the microcontroller is connected to your PC
        - $ ``

Specific commands to perform each step can be found in the following sections.

## Components
### Painting Algorithm Optimizer

A Rust toolchain is needed to compile the optimizer.
The code is located under the `algorithm` folder.

To compile and run it, execute:

```sh
cargo r --release -p algorithm -- -i inputs/mia.png
```
or else, to run it:

```sh
./algorithm -i inputs/mia.png
```

The optimizer will try multiple algorithms to get the optimal input sequence and minimize print time. The image is passed as an input parameter and must follow 1BPP PNG (1 Bit per Pixel - Indexed Mode of PNG format).

The optimal instruction list is generated in the project's folder: `instrs.rle`. Which is a run-length encoded list of instructions (button presses).

![Image of Mia Fey in 1bpp PNG format](docs/input.png)

All the options that are available can be checked by passing "-h" flag:
- -i, --input <INPUT>    [default: ]
- -o, --output <OUTPUT>  [default: instrs.rle]
- -f, --fast
- -h, --help             Print help
- -V, --version          Print version

### Simulator

The simulator creates a GIF animation given an instructions file.
It can be run with the following command:

`cargo r --release -p simulator -- -v -d -f 16`

or

`./simulator -i instrs.rle -v -f 32` if you use the prebuilt binary.

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

To compile the program for the embedded target, a Rust Toolchain for ARMv6-M (RP2040 Cortex-M0) is needed. Also, we need to install two tools, flip-link (for stack protection), and probe-rs-tools, which contains the tools to flash and debug (if needed) the binary into the microcontroller.

The toolchain and tools can be installed with the following commands:

```sh
rustup target install thumbv6m-none-eabi
cargo install flip-link
cargo install --locked probe-rs-tools
```

After that, to compile the program, it is required to change directory into the rp2040-impl folder, build the binary (in ELF format) and patch it with the desired instruction list.

Regardless of the method of generating the binary for the embedded target, it is required to embed the instruction list (image) into the binary. A python script is provided. It will search for the special `.instrs` section in the binary and then overwrite its contents with the instruction list.

The following commands build and patch the executable. It is assumed, the optimizer (`algorithm` module) has been run and a valid `instrs.rle` file exists in the project root folder.

```sh
cd rp2040-impl
cargo b --release
cd ..
python embed_instrs.py target/thumbv6m-none-eabi/release/rp2040-impl instrs.rle -o rp2040-impl.elf
```

Flash the binary file into the microcontroller with probe-rs

```sh
probe-rs run --chip RP2040 rp2040-impl.elf
```
Or drag and drop the UF2 file into the microcontroller after executing:
```sh
elf2uf2-rs rp2040-impl.elf rp2040-impl.uf2
```
