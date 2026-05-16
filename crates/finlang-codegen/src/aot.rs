//! AOT (ahead-of-time) compilation engine.
//!
//! Uses [`cranelift_object::ObjectModule`] to compile FinLang IR into a
//! relocatable object file suitable for linking with the system linker.
//!
//! Stdlib symbols are declared as [`cranelift_module::Linkage::Import`]; the
//! caller is responsible for linking the resulting object against the
//! `finlang_stdlib` static archive.  No in-process symbol resolution happens
//! here — that is the JIT's domain.

use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::settings::Configurable;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_module::{Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use finlang_ir::IrProgram;
use target_lexicon::Triple;

use crate::common::{make_isa, CodegenError};
use crate::translate::{ir_sig_to_clif, translate_function};

/// An AOT compilation engine that emits a linkable object file.
///
/// Construct with [`AotEngine::new`], then call [`AotEngine::compile`] to
/// obtain the raw object-file bytes.
pub struct AotEngine {
    module: ObjectModule,
    ctx: cranelift_codegen::Context,
    fb_ctx: FunctionBuilderContext,
}

impl AotEngine {
    /// Construct a new [`AotEngine`] targeting the given [`Triple`].
    ///
    /// `name` is used as the object module name and appears in debug
    /// information (typically the source file stem).
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError::IsaSetup`] if the target triple is not
    /// supported or the ISA flags are rejected.
    ///
    /// Returns [`CodegenError::ModuleError`] if the object builder cannot
    /// be initialised.
    pub fn new(triple: Triple, name: &str) -> Result<Self, CodegenError> {
        // For AOT we cannot use cranelift_native::builder() (which always
        // targets the host) when a cross-compilation triple is requested.
        // For the host triple, we reuse make_isa() for consistency.
        let isa: std::sync::Arc<dyn TargetIsa> = if triple == Triple::host() {
            make_isa()?
        } else {
            // Cross-compilation path: build an ISA for the given triple.
            let mut flag_builder = cranelift_codegen::settings::builder();
            flag_builder
                .set("opt_level", "speed")
                .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;
            flag_builder
                .set("use_colocated_libcalls", "false")
                .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;
            flag_builder
                .set("is_pic", "false")
                .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;
            let isa_builder = cranelift_codegen::isa::lookup(triple)
                .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;
            isa_builder
                .finish(cranelift_codegen::settings::Flags::new(flag_builder))
                .map_err(|e| CodegenError::IsaSetup(e.to_string()))?
        };

        let obj_builder = ObjectBuilder::new(
            isa,
            name.to_owned(),
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

        let module = ObjectModule::new(obj_builder);
        let ctx = module.make_context();
        let fb_ctx = FunctionBuilderContext::new();

        Ok(Self { module, ctx, fb_ctx })
    }

    /// Compile an [`IrProgram`] and return the raw object-file bytes.
    ///
    /// The bytes can be written directly to a `.o` file and linked with:
    ///
    /// ```text
    /// cc program.o -L/path/to/finlang-stdlib -lfinlang_stdlib -lm -o program
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError::ModuleError`] on any Cranelift module or
    /// object-emission error.
    ///
    /// Returns [`CodegenError::Internal`] on internal consistency failures.
    pub fn compile(&mut self, program: &IrProgram) -> Result<Vec<u8>, CodegenError> {
        // ── Declare all functions ─────────────────────────────────────────────
        let mut func_ids = Vec::with_capacity(program.functions.len());
        for func in &program.functions {
            let sig = ir_sig_to_clif(func, &self.module);
            let id = self
                .module
                .declare_function(&func.name, Linkage::Export, &sig)
                .map_err(|e| CodegenError::ModuleError(e.to_string()))?;
            func_ids.push(id);
        }

        // ── Translate and define each function ────────────────────────────────
        for (func, &func_id) in program.functions.iter().zip(func_ids.iter()) {
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

        // ── Emit the object file ──────────────────────────────────────────────
        // `ObjectModule::finish` consumes the module, so we replace `self.module`
        // with a fresh one after emission.  We achieve this by using a local
        // helper to rebuild it; the engine can be reused after `compile` returns.
        let product = take_and_rebuild_module(&mut self.module, &mut self.ctx, &mut self.fb_ctx)?;
        let bytes = product
            .object
            .write()
            .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

        Ok(bytes)
    }
}

/// Consume the current [`ObjectModule`] to obtain the finished product, then
/// rebuild a fresh module in its place so the engine can be reused.
///
/// This is necessary because [`ObjectModule::finish`] takes `self` by value.
fn take_and_rebuild_module(
    module: &mut ObjectModule,
    ctx: &mut cranelift_codegen::Context,
    fb_ctx: &mut FunctionBuilderContext,
) -> Result<cranelift_object::ObjectProduct, CodegenError> {
    // We need to swap out the module.  Build a dummy replacement module for the
    // same target so we always leave `self.module` in a valid state.
    let isa = make_isa()?;
    let dummy_builder = ObjectBuilder::new(
        isa,
        "dummy".to_owned(),
        cranelift_module::default_libcall_names(),
    )
    .map_err(|e| CodegenError::ModuleError(e.to_string()))?;
    let new_module = ObjectModule::new(dummy_builder);
    let old_module = std::mem::replace(module, new_module);
    *ctx = module.make_context();
    *fb_ctx = FunctionBuilderContext::new();
    Ok(old_module.finish())
}
