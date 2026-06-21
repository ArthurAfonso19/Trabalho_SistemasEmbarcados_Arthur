# Agent Notes

## Repo Shape
- Single-crate embedded Rust firmware; the only application entrypoint is `src/main.rs`.
- Target is fixed in `.cargo/config.toml` to `thumbv7em-none-eabihf`; this repo is not set up for host builds.
- `cargo run` is wired to hardware flashing, not a desktop run: the runner is `probe-rs run --chip STM32G474RETx`.

## Commands
- `cargo check` verifies cross-compilation without flashing hardware.
- `cargo run` builds and flashes the STM32G474RE target through `probe-rs`.
- `cargo fmt --check` is the obvious formatting check; no repo-local lint/test/CI config exists.

## Firmware Wiring
- `build.rs` always injects `--nmagic`, `-Tlink.x`, and `-Tdefmt.x`; avoid duplicating linker flags elsewhere unless you mean to change the whole build.
- `Embed.toml` is configured for `STM32G474RETx` with RTT enabled, and `.cargo/config.toml` sets `DEFMT_LOG=info` by default.
- `src/main.rs` uses Embassy async tasks and spawns ADC, EXTI button, LPUART1 echo, TIM1 PWM, and ADXL345 I2C tasks before entering the PA5 blink loop.

## Memory/Layout Gotchas
- `memory.x` defines extra SRAM sections `.data2` in `SRAM2` and `.ccmdata` in `CCMRAM`.
- `src/main.rs` has a `#[pre_init]` routine that manually copies those custom sections from flash before `main`; if you rename/remove those sections, update both `memory.x` and the inline assembly together.

## Hardware Assumptions
- Clock setup in `main` assumes a 24 MHz HSE and configures PLL1 to a 170 MHz system clock; peripheral timing changes should be checked against that configuration.
- The firmware is board-specific by pin usage in `main` (`PA5` LED, `PC13` EXTI button, `LPUART1` on `PA3/PA2`, `I2C1` on `PB8/PB9`, PWM on `PC0`, ADC on `PA7`).
