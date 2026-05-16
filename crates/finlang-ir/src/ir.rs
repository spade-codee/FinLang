//! SSA IR data types.
//!
//! The central types here are [`IrProgram`], [`IrFunction`], [`BasicBlock`],
//! and [`Inst`].  Everything is `Clone` so the optimiser can work on owned
//! copies without lifetime entanglement.

use finlang_parser::ast::{BinOpKind, UnaryOpKind};

// ── Identifiers ───────────────────────────────────────────────────────────────

/// A numeric SSA value identifier, unique within a single function.
///
/// `ValueId(0)` is conventionally the first allocated value; counters are
/// allocated strictly in order by [`crate::lower`]'s `FunctionBuilder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ValueId(pub u32);

/// A basic-block identifier, unique within a single function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BlockId(pub u32);

// ── Types ─────────────────────────────────────────────────────────────────────

/// Lowered primitive types — the complete set that codegen needs in order to
/// pick the right Cranelift type for every SSA value.
///
/// All FinLang financial dimensions (`Price`, `Rate`, `Notional`, `Years`,
/// `BasisPoints`) collapse to [`IrType::F64`] because they share the same
/// runtime representation.  `Int`, `Date`, and `OptionType` become
/// [`IrType::I64`].  `Bool` maps to an `i8` at the Cranelift layer but is
/// kept distinct here so constant-folding can carry the correct semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    /// 64-bit IEEE-754 double — used for every financial numeric dimension.
    F64,
    /// 64-bit signed integer — `Int`, `Date`, and `OptionType` discriminants.
    I64,
    /// Boolean (`i8` at the Cranelift layer, `0` or `1`).
    Bool,
}

// ── Instructions ──────────────────────────────────────────────────────────────

/// A single SSA instruction in a basic block.
///
/// Every instruction that defines a value carries a `dst: ValueId` field.
/// The type of each `dst` is recorded in [`IrFunction::value_types`] when
/// the instruction is emitted by the lowering pass.
#[derive(Debug, Clone, PartialEq)]
pub enum Inst {
    /// Define an `f64` constant.
    ConstF64 {
        /// Destination value.
        dst: ValueId,
        /// The constant value.
        value: f64,
    },
    /// Define an `i64` constant.
    ConstI64 {
        /// Destination value.
        dst: ValueId,
        /// The constant value.
        value: i64,
    },
    /// Define a boolean constant.
    ConstBool {
        /// Destination value.
        dst: ValueId,
        /// The constant value.
        value: bool,
    },
    /// Binary operation.
    ///
    /// The [`BinOpKind`] is taken directly from the parser AST so that codegen
    /// can pattern-match on the same enum without a translation step.  The
    /// type checker has already verified dimensional correctness, so the IR
    /// treats the op as dimensionless.
    BinOp {
        /// Destination value (receives the result).
        dst: ValueId,
        /// The operator.
        op: BinOpKind,
        /// Left operand.
        lhs: ValueId,
        /// Right operand.
        rhs: ValueId,
    },
    /// Unary operation (negation or logical-not).
    UnaryOp {
        /// Destination value.
        dst: ValueId,
        /// The operator.
        op: UnaryOpKind,
        /// The operand.
        operand: ValueId,
    },
    /// Cast an `i64` value to `f64`.
    ///
    /// Emitted when an `Int`-typed expression is used in an `f64` context
    /// (e.g. `period as rate` in bond arithmetic).
    CastIntToFloat {
        /// Destination `f64` value.
        dst: ValueId,
        /// Source `i64` value.
        src: ValueId,
    },
    /// Cast an `f64` value to `i64` (truncating towards zero).
    ///
    /// Emitted for explicit `as int` casts in source code.
    CastFloatToInt {
        /// Destination `i64` value.
        dst: ValueId,
        /// Source `f64` value.
        src: ValueId,
    },
    /// Call an external stdlib function by ABI name.
    ///
    /// `callee` is the mangled symbol (e.g. `"finlang_black_scholes"`).
    /// A `None` `dst` denotes a void call; the FinLang stdlib currently has no
    /// void functions but the field is reserved for completeness.
    Call {
        /// Optional destination value for the return.
        dst: Option<ValueId>,
        /// ABI symbol name.
        callee: String,
        /// Argument values, in call order.
        args: Vec<ValueId>,
    },
    /// Return from the current function.
    Return {
        /// The return value, or `None` for a void return.
        value: Option<ValueId>,
    },
    /// Conditional branch — jumps to `then_block` if `cond` is `true`,
    /// otherwise to `else_block`.
    Branch {
        /// The boolean condition value.
        cond: ValueId,
        /// Target block for the true arm.
        then_block: BlockId,
        /// Target block for the false arm.
        else_block: BlockId,
    },
    /// Unconditional jump to another block.
    Jump {
        /// Target block.
        target: BlockId,
    },
    /// SSA φ-node at the start of a join block.
    ///
    /// `dst` takes the value of `incoming[predecessor_block]` when control
    /// arrives from that predecessor.  The predecessor blocks must be the
    /// exact set of predecessors of the block containing this instruction.
    Phi {
        /// Destination value (the merged result).
        dst: ValueId,
        /// `(value_from_predecessor, predecessor_block_id)` pairs.
        incoming: Vec<(ValueId, BlockId)>,
    },
}

impl Inst {
    /// Return the `dst` `ValueId` defined by this instruction, if any.
    ///
    /// Terminators (`Return`, `Branch`, `Jump`) and calls with `dst: None`
    /// define no value; this returns `None` for those cases.
    #[must_use]
    pub fn dst(&self) -> Option<ValueId> {
        match self {
            Inst::ConstF64 { dst, .. }
            | Inst::ConstI64 { dst, .. }
            | Inst::ConstBool { dst, .. }
            | Inst::BinOp { dst, .. }
            | Inst::UnaryOp { dst, .. }
            | Inst::CastIntToFloat { dst, .. }
            | Inst::CastFloatToInt { dst, .. }
            | Inst::Phi { dst, .. } => Some(*dst),
            Inst::Call { dst, .. } => *dst,
            Inst::Return { .. } | Inst::Branch { .. } | Inst::Jump { .. } => None,
        }
    }

    /// Return all `ValueId` operands **read** by this instruction (not the `dst`).
    #[must_use]
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Inst::ConstF64 { .. } | Inst::ConstI64 { .. } | Inst::ConstBool { .. } => vec![],
            Inst::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
            Inst::UnaryOp { operand, .. } => vec![*operand],
            Inst::CastIntToFloat { src, .. } | Inst::CastFloatToInt { src, .. } => vec![*src],
            Inst::Call { args, .. } => args.clone(),
            Inst::Return { value } => value.iter().copied().collect(),
            Inst::Branch { cond, .. } => vec![*cond],
            Inst::Jump { .. } => vec![],
            Inst::Phi { incoming, .. } => incoming.iter().map(|(v, _)| *v).collect(),
        }
    }

    /// Return `true` if this instruction is a block terminator.
    #[must_use]
    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Inst::Return { .. } | Inst::Branch { .. } | Inst::Jump { .. }
        )
    }
}

// ── Basic block ───────────────────────────────────────────────────────────────

/// A sequence of instructions forming a basic block.
///
/// Every block must end with exactly one terminator instruction
/// (`Return`, `Branch`, or `Jump`).  This invariant is checked by
/// [`crate::validate::validate_ssa`].
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// Unique block identifier within the enclosing function.
    pub id: BlockId,
    /// Instructions in program order.  The last instruction must be a
    /// terminator.
    pub insts: Vec<Inst>,
}

// ── Function ──────────────────────────────────────────────────────────────────

/// A single lowered function in SSA form.
///
/// The parameters are represented as `(name, value_id, ir_type)` triples.
/// The `value_id`s of parameters are pre-allocated at entry to
/// `FunctionBuilder::new` and stored in [`IrFunction::value_types`] alongside
/// all other defined values.
#[derive(Debug, Clone)]
pub struct IrFunction {
    /// The function name (e.g. `"__main__"` or user-defined name).
    pub name: String,
    /// Parameter descriptors: `(source_name, ssa_value, ir_type)`.
    pub params: Vec<(String, ValueId, IrType)>,
    /// Return type of the function.
    pub return_ty: IrType,
    /// Basic blocks, in the order they were created.
    pub blocks: Vec<BasicBlock>,
    /// The entry basic block.
    pub entry: BlockId,
    /// Type of every SSA value defined in this function.
    ///
    /// Indexed by `ValueId.0`.  This vec grows monotonically as the lowering
    /// pass allocates new values.  Reading `value_types[v.0]` is always safe
    /// for any `v` that was returned by the builder.
    pub value_types: Vec<IrType>,
}

// ── Program ───────────────────────────────────────────────────────────────────

/// A fully lowered FinLang program: a collection of [`IrFunction`]s.
///
/// The synthetic `__main__` function (if present) is always `functions[0]`.
#[derive(Debug, Clone)]
pub struct IrProgram {
    /// All functions in the program, in lowering order.
    pub functions: Vec<IrFunction>,
}
