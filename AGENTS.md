# Agent Notes

## Commands
- `cargo check` is the safe verification command; it cross-compiles for `thumbv7em-none-eabihf` without flashing hardware.
- `cargo run` is not a host run. `.cargo/config.toml` routes it to `probe-rs run --chip STM32G474RETx`.
- `cargo fmt --check` is the only repo-local formatting check. There is no repo-local test, lint, or CI config.

## Structure
- This is a single-crate embedded firmware repo. The only binary entrypoint is `src/main.rs`.
- `src/main.rs` owns the board wiring and runtime setup; `src/app/shell.rs` is the only extracted feature module and is spawned from `main` as the UART shell task.

## Wiring Gotchas
- `build.rs` always injects `--nmagic`, `-Tlink.x`, and `-Tdefmt.x`; do not duplicate or partially move those linker flags unless you intend to change the whole link setup.
- `.cargo/config.toml` sets `DEFMT_LOG=info`. `Embed.toml` is pinned to `STM32G474RETx` with RTT enabled.
- `memory.x` defines custom `.data2` (SRAM2) and `.ccmdata` (CCMRAM) sections. `src/main.rs` has a `#[pre_init]` routine that manually copies both from flash before `main`; changes to those section names or layout must be kept in sync across both files.

## Hardware Assumptions
- `stm32_config()` assumes a 24 MHz external HSE and drives the system clock from PLL1 at 170 MHz; peripheral timing work should be checked against that clock tree.
- The firmware is board-specific by hardcoded pins in `main`: `PA5` LED blink loop, `PC13` EXTI button, `LPUART1` on `PA3/PA2`, `I2C1` on `PB8/PB9`, PWM on `PC0`, ADC on `PA7`.
- `main` performs startup self-checks, then spawns ADC, button, shell UART, PWM, and ADXL345 tasks before entering the infinite LED loop. Changes to peripheral ownership usually need to be coordinated there.
