//! Dead-code elimination pass.
//!
//! [`dce`] uses a worklist-based live-value analysis to remove instructions
//! whose results are never used.  The algorithm is:
//!
//! 1. **Seed**: every value used by a `Return`, `Branch`, or `Call` (calls
//!    have observable side effects and are always kept) is placed in `live`.
//! 2. **Propagate**: while the worklist is non-empty, pop a live value, find
//!    the instruction that defines it, mark its operands live, and add them to
//!    the worklist.
//! 3. **Sweep**: remove any instruction whose `dst` is not live.  Instructions
//!    without a `dst` (terminators) and `Call` with `dst: None` are kept
//!    unconditionally.
//!
//! After the sweep a debug-mode `validate_ssa` call confirms the IR is still
//! structurally sound.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::ir::{Inst, IrFunction, IrProgram, ValueId};
use crate::validate::validate_ssa;

// ── Public entry point ────────────────────────────────────────────────────────

/// Eliminate dead instructions from all functions in `program`.
///
/// An instruction is considered dead if its result value is never used by
/// any live instruction.  `Call` instructions are always considered live
/// (observable side effects).  After elimination a structural SSA validity
/// check is run in debug builds.
pub fn dce(program: &mut IrProgram) {
    for func in &mut program.functions {
        dce_function(func);
    }
    debug_assert!(
        validate_ssa(program).is_ok(),
        "SSA invariant violated after DCE"
    );
}

fn dce_function(func: &mut IrFunction) {
    // ── Step 1: Build def map (value → instruction index in flat list) ─────────
    // We also build a flat use-def: for each value, which operands does it use?
    let mut def_inst: HashMap<ValueId, (usize, usize)> = HashMap::new(); // (block_idx, inst_idx)
    for (bi, block) in func.blocks.iter().enumerate() {
        for (ii, inst) in block.insts.iter().enumerate() {
            if let Some(dst) = inst.dst() {
                def_inst.insert(dst, (bi, ii));
            }
        }
    }

    // ── Step 2: Seed live set ──────────────────────────────────────────────────
    let mut live: HashSet<ValueId> = HashSet::new();
    let mut worklist: VecDeque<ValueId> = VecDeque::new();

    for block in func.blocks.iter() {
        for inst in &block.insts {
            let always_live = match inst {
                // Terminators use values → seed them.
                Inst::Return { .. } | Inst::Branch { .. } => true,
                // Calls have side effects → keep regardless of dst.
                Inst::Call { .. } => true,
                _ => false,
            };
            if always_live {
                for op in inst.operands() {
                    if live.insert(op) {
                        worklist.push_back(op);
                    }
                }
                // If a Call has a dst, that dst is produced by a live instr
                // but we don't need to separately mark it live here — we
                // keep the Call instruction unconditionally in the sweep.
            }
        }
    }

    // ── Step 3: Propagate ──────────────────────────────────────────────────────
    while let Some(v) = worklist.pop_front() {
        if let Some(&(bi, ii)) = def_inst.get(&v) {
            let inst = &func.blocks[bi].insts[ii];
            for op in inst.operands() {
                if live.insert(op) {
                    worklist.push_back(op);
                }
            }
        }
    }

    // ── Step 4: Sweep ──────────────────────────────────────────────────────────
    for block in &mut func.blocks {
        block.insts.retain(|inst| {
            // Terminators: always keep.
            if inst.is_terminator() {
                return true;
            }
            // Calls: always keep (side effects).
            if matches!(inst, Inst::Call { .. }) {
                return true;
            }
            // Any instruction defining a live value: keep.
            if let Some(dst) = inst.dst() {
                return live.contains(&dst);
            }
            // Instruction with no dst and not a terminator/call: remove.
            false
        });
    }
}
