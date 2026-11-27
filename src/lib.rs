//! snes-recomp as a library: the static recompiler (ROM, disassembly, discovery, graph, C generator), the game
//! profiles, the port builder and the generation of a game's C in memory (`generate`), which is what the command line
//! programs use and what a build on another device (Android) calls through its C interface (`generate::ffi`).

pub mod codegen;
#[cfg(test)]
mod cctest;
pub mod config;
pub mod crc32;
pub mod disasm;
pub mod discover;
pub mod download;
pub mod games;
pub mod generate;
pub mod graph;
#[cfg(test)]
mod headless;
pub mod launcher;
#[cfg(windows)]
pub mod launcher_win;
pub mod opcodes;
pub mod port;
pub mod rom;
#[cfg(test)]
mod rtlib;
#[cfg(test)]
mod spctest;
pub mod toolchain;
pub mod update;
pub mod ui;

/// The commit this program is built from (`build.rs`: short hash, `-dirty` with changes, `unknown` outside git).
pub const ENGINE_COMMIT: &str = env!("SNES_ENGINE_COMMIT");
/// `--version` and the line under the launcher: the version of the crate and the commit.
pub const ENGINE_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("SNES_ENGINE_COMMIT"), ")");
