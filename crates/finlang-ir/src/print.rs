//! Pretty-printer for SSA IR.
//!
//! Implements [`std::fmt::Display`] for [`IrProgram`] and [`IrFunction`].
//! The output format is stable enough for snapshot tests: block labels use
//! `bb<n>:`, values use `v<n>`, constants use `const.f64` / `const.i64` /
//! `const.bool`, and binary ops use `binop.<op>`.

use std::fmt;

use finlang_parser::ast::{BinOpKind, UnaryOpKind};

use crate::ir::{BlockId, Inst, IrFunction, IrProgram, IrType, ValueId};

// ── IrProgram ─────────────────────────────────────────────────────────────────

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, func) in self.functions.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{func}")?;
        }
        Ok(())
    }
}

// ── IrFunction ────────────────────────────────────────────────────────────────

impl fmt::Display for IrFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Signature: fn NAME(params) -> ret_ty {
        write!(f, "fn {}(", self.name)?;
        for (i, (name, vid, ty)) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {} {}", name, fmt_value(*vid), fmt_type(*ty))?;
        }
        writeln!(f, ") -> {} {{", fmt_type(self.return_ty))?;

        for block in &self.blocks {
            // Block label — two-space indent.
            writeln!(f, "  {}:", fmt_block(block.id))?;
            for inst in &block.insts {
                // Instruction — four-space indent.
                writeln!(f, "    {}", fmt_inst(inst))?;
            }
        }

        write!(f, "}}")
    }
}

// ── Instruction formatting ────────────────────────────────────────────────────

fn fmt_inst(inst: &Inst) -> String {
    match inst {
        Inst::ConstF64 { dst, value } => {
            format!("{} = const.f64 {value}", fmt_value(*dst))
        }
        Inst::ConstI64 { dst, value } => {
            // Annotate well-known discriminants.
            let comment = match value {
                0 => " ; Call",
                1 => " ; Put",
                _ => "",
            };
            format!("{} = const.i64 {value}{comment}", fmt_value(*dst))
        }
        Inst::ConstBool { dst, value } => {
            format!("{} = const.bool {value}", fmt_value(*dst))
        }
        Inst::BinOp { dst, op, lhs, rhs } => {
            format!(
                "{} = binop.{} {}, {}",
                fmt_value(*dst),
                fmt_binop(*op),
                fmt_value(*lhs),
                fmt_value(*rhs)
            )
        }
        Inst::UnaryOp { dst, op, operand } => {
            format!(
                "{} = unaryop.{} {}",
                fmt_value(*dst),
                fmt_unaryop(*op),
                fmt_value(*operand)
            )
        }
        Inst::CastIntToFloat { dst, src } => {
            format!("{} = cast.itof {}", fmt_value(*dst), fmt_value(*src))
        }
        Inst::CastFloatToInt { dst, src } => {
            format!("{} = cast.ftoi {}", fmt_value(*dst), fmt_value(*src))
        }
        Inst::Call { dst, callee, args } => {
            let args_s: Vec<String> = args.iter().map(|v| fmt_value(*v)).collect();
            let args_joined = args_s.join(", ");
            match dst {
                Some(d) => format!("{} = call {callee}({args_joined})", fmt_value(*d)),
                None => format!("call {callee}({args_joined})"),
            }
        }
        Inst::Return { value } => match value {
            Some(v) => format!("return {}", fmt_value(*v)),
            None => "return".to_owned(),
        },
        Inst::Branch { cond, then_block, else_block } => {
            format!(
                "branch {} ? {} : {}",
                fmt_value(*cond),
                fmt_block(*then_block),
                fmt_block(*else_block)
            )
        }
        Inst::Jump { target } => format!("jump {}", fmt_block(*target)),
        Inst::Phi { dst, incoming } => {
            let parts: Vec<String> = incoming
                .iter()
                .map(|(v, b)| format!("[{}, {}]", fmt_value(*v), fmt_block(*b)))
                .collect();
            format!("{} = phi {}", fmt_value(*dst), parts.join(", "))
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fmt_value(v: ValueId) -> String {
    format!("v{}", v.0)
}

fn fmt_block(b: BlockId) -> String {
    format!("bb{}", b.0)
}

fn fmt_type(t: IrType) -> &'static str {
    match t {
        IrType::F64 => "f64",
        IrType::I64 => "i64",
        IrType::Bool => "bool",
    }
}

fn fmt_binop(op: BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "add",
        BinOpKind::Sub => "sub",
        BinOpKind::Mul => "mul",
        BinOpKind::Div => "div",
        BinOpKind::Mod => "mod",
        BinOpKind::Eq => "eq",
        BinOpKind::NotEq => "neq",
        BinOpKind::Lt => "lt",
        BinOpKind::Gt => "gt",
        BinOpKind::LtEq => "lteq",
        BinOpKind::GtEq => "gteq",
        BinOpKind::And => "and",
        BinOpKind::Or => "or",
    }
}

fn fmt_unaryop(op: UnaryOpKind) -> &'static str {
    match op {
        UnaryOpKind::Neg => "neg",
        UnaryOpKind::Not => "not",
    }
}
