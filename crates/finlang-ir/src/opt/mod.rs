//! Optimisation passes for the SSA IR.
//!
//! Two passes are provided:
//!
//! * [`const_fold`] — constant-folding to a fixed point.
//! * [`dce`] — dead-code elimination via a live-value worklist.
//!
//! Both passes operate on `&mut IrProgram` and mutate the IR in place without
//! allocating a new program.

mod const_fold;
mod dce;

pub use const_fold::const_fold;
pub use dce::dce;
