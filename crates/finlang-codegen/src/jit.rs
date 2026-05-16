//! JIT compilation engine.
//!
//! Uses [`cranelift_jit::JITModule`] to compile FinLang IR in-process,
//! immediately making the generated machine code callable.
//!
//! # Unsafe code
//!
//! This module contains the **only** `unsafe` block in the entire crate.
//! After Cranelift finalises a function it returns a raw `*const u8` pointer
//! to the generated machine code.  We must transmute that pointer to a typed
//! `extern "C" fn()` before calling it.  Each unsafe block carries an explicit
//! `// SAFETY:` comment explaining the invariants that make the transmute sound.

#[allow(unsafe_code)]
mod inner {
    use cranelift_codegen::Context as ClifContext;
    use cranelift_frontend::FunctionBuilderContext;
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    use finlang_ir::{IrProgram, IrType};

    use crate::common::{make_isa, stdlib_symbols, CodegenError};
    use crate::translate::{ir_sig_to_clif, translate_function};

    // ── JitEngine ─────────────────────────────────────────────────────────────

    /// A Cranelift JIT engine for FinLang programs.
    ///
    /// Call [`JitEngine::compile`] to lower an [`IrProgram`] to native code,
    /// then call [`JitProgram::run`] on the result to execute `__main__`.
    pub struct JitEngine {
        module: JITModule,
        ctx: ClifContext,
        fb_ctx: FunctionBuilderContext,
    }

    impl JitEngine {
        /// Construct a new [`JitEngine`].
        ///
        /// Builds the native ISA, registers all stdlib symbols, and creates the
        /// [`JITModule`].
        ///
        /// # Errors
        ///
        /// Returns [`CodegenError::IsaSetup`] if the host platform is not
        /// supported by Cranelift.
        pub fn new() -> Result<Self, CodegenError> {
            let isa = make_isa()?;
            let mut jit_builder = JITBuilder::with_isa(
                isa,
                cranelift_module::default_libcall_names(),
            );
            // Register every finlang_* stdlib symbol as an in-process pointer.
            for (name, ptr) in stdlib_symbols() {
                jit_builder.symbol(name, ptr);
            }
            let module = JITModule::new(jit_builder);
            let ctx = module.make_context();
            let fb_ctx = FunctionBuilderContext::new();
            Ok(Self { module, ctx, fb_ctx })
        }

        /// Compile an [`IrProgram`] to native code and return a [`JitProgram`]
        /// ready to execute.
        ///
        /// # Errors
        ///
        /// Returns [`CodegenError::MainNotFound`] if the program does not
        /// contain a function named `__main__`.
        ///
        /// Returns [`CodegenError::ModuleError`] on any Cranelift module error.
        pub fn compile(&mut self, program: &IrProgram) -> Result<JitProgram, CodegenError> {
            // ── Declare all functions first so forward calls resolve. ─────────
            let mut func_ids = Vec::with_capacity(program.functions.len());
            for func in &program.functions {
                let sig = ir_sig_to_clif(func, &self.module);
                let id = self
                    .module
                    .declare_function(&func.name, Linkage::Export, &sig)
                    .map_err(|e| CodegenError::ModuleError(e.to_string()))?;
                func_ids.push(id);
            }

            // ── Translate and define each function. ───────────────────────────
            for (func, &func_id) in program.functions.iter().zip(func_ids.iter()) {
                // Set the signature on the context.
                self.ctx.func.signature = ir_sig_to_clif(func, &self.module);

                translate_function(
                    func,
                    program,
                    &mut self.module,
                    func_id,
                    &mut self.ctx,
                    &mut self.fb_ctx,
                )?;

                self.module
                    .define_function(func_id, &mut self.ctx)
                    .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

                self.module.clear_context(&mut self.ctx);
            }

            // ── Finalise all definitions (patches relocations). ───────────────
            self.module.finalize_definitions()
                .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

            // ── Locate __main__. ──────────────────────────────────────────────
            let main_idx = program
                .functions
                .iter()
                .position(|f| f.name == "__main__")
                .ok_or(CodegenError::MainNotFound)?;

            let main_id = func_ids[main_idx];
            let main_ptr = self.module.get_finalized_function(main_id);
            let main_ty = program.functions[main_idx].return_ty;

            Ok(JitProgram { main_ptr, main_ty })
        }
    }

    // ── JitProgram ────────────────────────────────────────────────────────────

    /// A compiled FinLang program ready for execution.
    ///
    /// Created by [`JitEngine::compile`]; consumed by calling [`JitProgram::run`].
    pub struct JitProgram {
        /// Raw pointer to the compiled `__main__` code.
        pub(super) main_ptr: *const u8,
        /// The declared return type of `__main__`.
        pub(super) main_ty: IrType,
    }

    impl JitProgram {
        /// Execute `__main__` and return its scalar result.
        ///
        /// The return type drives which function-pointer signature is used for
        /// the call:
        ///
        /// * [`IrType::F64`]  → `extern "C" fn() -> f64`
        /// * [`IrType::I64`]  → `extern "C" fn() -> i64`
        /// * [`IrType::Bool`] → `extern "C" fn() -> i8` (0 = false, 1 = true)
        pub fn run(&self) -> ScalarValue {
            match self.main_ty {
                IrType::F64 => {
                    // SAFETY: `main_ptr` was produced by
                    // `JITModule::get_finalized_function` after a successful
                    // `finalize_definitions`.  Cranelift guarantees the pointer
                    // is executable, non-null, and has been relocated against
                    // all declared symbols.  The signature `() -> f64` exactly
                    // matches the Cranelift signature set on the function
                    // (`ir_sig_to_clif` for a zero-param f64-returning function),
                    // and `extern "C"` matches the default calling convention
                    // used by `JITModule` on all supported targets.
                    let f: extern "C" fn() -> f64 =
                        unsafe { std::mem::transmute(self.main_ptr) };
                    ScalarValue::F64(f())
                }
                IrType::I64 => {
                    // SAFETY: same as the F64 branch above; the signature
                    // `() -> i64` matches the declared Cranelift function
                    // signature for an i64-returning __main__.
                    let f: extern "C" fn() -> i64 =
                        unsafe { std::mem::transmute(self.main_ptr) };
                    ScalarValue::I64(f())
                }
                IrType::Bool => {
                    // SAFETY: same as the F64 branch above; the signature
                    // `() -> i8` matches the declared Cranelift function
                    // signature for a bool (i8)-returning __main__.  A non-zero
                    // return value is treated as true.
                    let f: extern "C" fn() -> i8 =
                        unsafe { std::mem::transmute(self.main_ptr) };
                    ScalarValue::Bool(f() != 0)
                }
            }
        }
    }

    // ── ScalarValue ───────────────────────────────────────────────────────────

    /// A runtime-typed scalar returned by [`JitProgram::run`].
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum ScalarValue {
        /// A 64-bit IEEE-754 double.
        F64(f64),
        /// A 64-bit signed integer.
        I64(i64),
        /// A boolean.
        Bool(bool),
    }
}

pub use inner::{JitEngine, JitProgram, ScalarValue};
