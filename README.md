# ram-probe-rs

`ram-probe-rs` is a ~~shameless rip-off~~ proof of concept heavily inspired by [`teleprobe`](https://github.com/embassy-rs/teleprobe) and based on [`probe-rs`](https://github.com/probe-rs/probe-rs) to upload RAM-only programs to ARM Cortex-M micro-controllers (MCUs).

**WARNING**: `ram-probe-rs` is unsupported and unmaintained. I don't have the time or knowledge to support a project like this. I'm only releasing this because I hope the Rust embedded eco-system will eventually have this capability. There is no guarantee any of this works; it was mainly a learning experience.

Currently, `probe-rs` assumes binaries will be flashed to the target, and can't run RAM-only programs:

* [support pure RAM programs (#160)](https://github.com/knurling-rs/probe-run/issues/160)
* [Run from Ram (#1884)](https://github.com/probe-rs/probe-rs/issues/1884)

Additionally, while `probe-rs` supports downloading data from hosts to targets, uploading data from targets to hosts is somewhat limited (although this can be done with RTT).

`teleprobe` can download RAM-only programs, but is complicated due to it's client and server mode, and doesn't expose a library.

## Creating a RAM-only program

As described in the [`teleprobe` README](https://github.com/embassy-rs/teleprobe), RAM-only programs can be created via a custom linker script. Instead of linking to e.g. the [`link.x`](https://github.com/rust-embedded/cortex-m/blob/master/cortex-m-rt/link.x.in) linker script provided by the [`cortex-m`](https://github.com/rust-embedded/cortex-m) crate, programs can be linked to the modified [`link_ram_cortex_m.x`](link_ram_cortex_m.x) linker script.

## Downloading a RAM-only program

The `ram-probe-cli` crate produces a binary named `ram-probe`, which is similar to `probe-rs`:

```bash
ram-probe --chip 'STM32F303RETx' ../ram-prog/target/thumbv7em-none-eabihf/debug/ram-prog
```

This currently requires a RAM-only ELF file with RTT and [`defmt`](https://github.com/knurling-rs/defmt) logging.

This will:

1. Parse the ELF file specified, to extract the program, RTT and `defmt` information.
1. Validate the program is RAM-only.
1. Connect to a debug probe.
1. Download the program from the host to the target.
1. Reset and initialize the MCU.
1. Establish RTT communication and log `defmt` messages.

The output can be tweaked with the `RUST_LOG` environmental variable, see [`env_logger`](https://docs.rs/env_logger/latest/env_logger/). The `defmt` output is written to the `target` logger, and so can be modified by e.g. `target=debug`.

An example with maximum logging:

```bash
env RUST_LOG="warn,ram_probe=trace,ram_probe_rs=trace,target=info" \
  ram-probe --chip 'STM32F303RETx' ../ram-prog/target/thumbv7em-none-eabihf/debug/ram-prog
```

## Uses of RAM-only programs

Why is this even interesting? RAM-only programs can be quite limited due to a target's RAM size, but still have useful properties.

### Faster iteration

RAM-only programs are very fast to download to a target. This makes testing code fast.

### Reverse engineering

RAM-only programs obviously don't alter a target's flash. After a reset, the RAM-only program is gone and the device is unmodified. This is useful for exploring devices for reverse engineering.

Also, RAM-only programs are very useful for interacting with peripherals such as external flash...

### Flashing

RAM-only programs are very common for flashing MCUs. If you think about it, this is obvious as most targets have more flash than RAM. So, to flash a program, it's required to write it in chunks.

ARM provides "flash programming algorithms" in a [CMSIS-Pack](https://open-cmsis-pack.github.io/Open-CMSIS-Pack-Spec/main/html/flashAlgorithm.html) for each target. These follow the [CMSIS algorithm specification](https://open-cmsis-pack.github.io/Open-CMSIS-Pack-Spec/main/html/algorithmFunc.html). These [are used](https://github.com/probe-rs/probe-rs/tree/master/target-gen) by `probe-rs`.

There are multiple names for this concept:

* `probe-rs` calls it a "(CMSIS-Pack) flash algorithm", aligned with ARM.
* [`OpenOCD`](https://openocd.org/) calls it a ["flash loader" or "ram loader](https://sourceforge.net/p/openocd/code/ci/master/tree/contrib/loaders/).
* Segger's J-Link/J-Flash calls it ["RAMCode"](https://wiki.segger.com/SPI_Flash#Indirect_programming).

Specifically J-Flash seems interesting, as it ostensibly supports something called "indirect mode". So, something like `ram-probe-rs` could open the way to an open source J-Flash alternative.

## License

This work is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
