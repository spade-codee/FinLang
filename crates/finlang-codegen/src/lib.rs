//! Cranelift-based JIT and AOT backend for the FinLang compiler.
//!
//! This crate translates FinLang SSA IR ([`finlang_ir::IrProgram`]) into native
//! x86-64 machine code using the Cranelift code-generation library.  Two
//! compilation modes are provided:
//!
//! * **JIT** ([`jit::JitEngine`]): compiles a program in-process using
//!   [`cranelift_jit::JITModule`] and immediately executes the resulting
//!   `__main__` function.  Used by the REPL and the test suite.
//!
//! * **AOT** ([`aot::AotEngine`]): compiles to a relocatable object file using
//!   [`cranelift_object::ObjectModule`] and returns the raw bytes.  The caller
//!   (e.g. `finlang compile`) links the object against the `finlang_stdlib`
//!   archive and the system C runtime.
//!
//! # Unsafe code policy
//!
//! This crate uses `#![deny(unsafe_code)]` at the root.  One narrow exception
//! exists in [`jit`]: after Cranelift finalises a function it gives back a raw
//! `*const u8` code pointer, and we must `transmute` that pointer to a typed
//! function pointer before calling it.  That single `unsafe` block is annotated
//! with a `// SAFETY:` comment explaining why the transmute is sound.  No other
//! unsafe code exists anywhere in this crate.
//!
//! # Stdlib symbol resolution
//!
//! The JIT registers every `finlang_*` symbol from [`finlang_stdlib`] via
//! [`cranelift_jit::JITBuilder::symbol`] before the module is created.  Because
//! those symbols carry `#[no_mangle]`, the address stored is the exact in-process
//! function pointer — zero FFI overhead at call sites.
//!
//! For AOT, the symbols are declared as [`cranelift_module::Linkage::Import`];
//! the linker resolves them when the user links against the stdlib archive.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod aot;
pub mod common;
pub mod jit;
pub mod translate;

pub use common::{ir_type_to_clif, stdlib_signature, stdlib_symbols, CodegenError};
pub use jit::{JitEngine, JitProgram, ScalarValue};
pub use aot::AotEngine;
