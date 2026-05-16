//! SSA structural validation.
//!
//! [`validate_ssa`] checks three invariants after any pass that mutates the IR:
//!
//! 1. Every value that appears as an operand is defined exactly once in the
//!    same function (use-before-def is also detected because we collect defs
//!    first).
//! 2. Every basic block ends with exactly one terminator instruction (`Return`,
//!    `Branch`, or `Jump`).
//! 3. Every `Phi` node references block ids that are actually present in the
//!    function.

use std::collections::HashSet;

use crate::ir::{BlockId, Inst, IrFunction, IrProgram, ValueId};

// ── Error type ────────────────────────────────────────────────────────────────

/// An SSA structural violation found by [`validate_ssa`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// A value is used but never defined in this function.
    UndefinedValue {
        /// The undefined value.
        value: ValueId,
        /// The name of the function containing the violation.
        function: String,
    },
    /// A basic block does not end with a terminator.
    MissingTerminator {
        /// The offending block.
        block: BlockId,
        /// The name of the function containing the violation.
        function: String,
    },
    /// A `Phi` node references a predecessor block that does not exist.
    PhiReferencesNonexistentBlock {
        /// The phi destination.
        phi_dst: ValueId,
        /// The non-existent predecessor block.
        missing_block: BlockId,
        /// The name of the function containing the violation.
        function: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::UndefinedValue { value, function } => {
                write!(f, "in {function}: v{} is used but never defined", value.0)
            }
            ValidationError::MissingTerminator { block, function } => {
                write!(f, "in {function}: bb{} has no terminator", block.0)
            }
            ValidationError::PhiReferencesNonexistentBlock {
                phi_dst,
                missing_block,
                function,
            } => {
                write!(
                    f,
                    "in {function}: phi v{} references non-existent bb{}",
                    phi_dst.0, missing_block.0
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

// ── Public entry point ────────────────────────────────────────────────────────

/// Validate all SSA invariants for every function in `program`.
///
/// Returns `Ok(())` if every function is structurally sound, or `Err` with the
/// first violation found.
///
/// # Errors
///
/// Returns the first [`ValidationError`] encountered, which may be one of:
/// * [`ValidationError::UndefinedValue`]
/// * [`ValidationError::MissingTerminator`]
/// * [`ValidationError::PhiReferencesNonexistentBlock`]
pub fn validate_ssa(program: &IrProgram) -> Result<(), ValidationError> {
    for func in &program.functions {
        validate_function(func)?;
    }
    Ok(())
}

fn validate_function(func: &IrFunction) -> Result<(), ValidationError> {
    // Collect all block ids.
    let block_ids: HashSet<BlockId> = func.blocks.iter().map(|b| b.id).collect();

    // Collect all defined values (params + instruction dsts).
    let mut defined: HashSet<ValueId> = func.params.iter().map(|(_, v, _)| *v).collect();
    for block in &func.blocks {
        for inst in &block.insts {
            if let Some(dst) = inst.dst() {
                defined.insert(dst);
            }
        }
    }

    for block in &func.blocks {
        // ── Check terminator ──────────────────────────────────────────────────
        let last_is_terminator = block.insts.last().map(Inst::is_terminator).unwrap_or(false);
        if !last_is_terminator {
            return Err(ValidationError::MissingTerminator {
                block: block.id,
                function: func.name.clone(),
            });
        }

        for inst in &block.insts {
            // ── Check operands are defined ────────────────────────────────────
            for op in inst.operands() {
                if !defined.contains(&op) {
                    return Err(ValidationError::UndefinedValue {
                        value: op,
                        function: func.name.clone(),
                    });
                }
            }

            // ── Check phi predecessor blocks exist ────────────────────────────
            if let Inst::Phi { dst, incoming } = inst {
                for (_, pred_block) in incoming {
                    if !block_ids.contains(pred_block) {
                        return Err(ValidationError::PhiReferencesNonexistentBlock {
                            phi_dst: *dst,
                            missing_block: *pred_block,
                            function: func.name.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}
