//! AOT compilation test.
//!
//! Compiles `option_pricing.fin` through the AOT engine and verifies that the
//! resulting bytes form a recognisable object-file header for the host platform.

mod common;

use finlang_codegen::AotEngine;
use target_lexicon::Triple;

#[test]
fn aot_emits_valid_object_file() {
    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/option_pricing.fin"),
    )
    .expect("read option_pricing.fin");

    let prog = common::compile_source(&src);

    let mut engine = AotEngine::new(Triple::host(), "option_pricing")
        .expect("AotEngine::new");

    let bytes = engine.compile(&prog).expect("aot compile");

    // The object file must be non-empty.
    assert!(!bytes.is_empty(), "object file should not be empty");

    // Write to a temp file so the tester can inspect it.
    let tmp = std::env::temp_dir().join("finlang_option_pricing_test.o");
    std::fs::write(&tmp, &bytes).expect("write object file");
    println!("object file written to: {}", tmp.display());
    println!("object file size: {} bytes", bytes.len());

    // Check the magic bytes for the host platform.
    #[cfg(target_os = "linux")]
    {
        // ELF magic: 0x7F 'E' 'L' 'F'
        assert_eq!(
            &bytes[..4],
            &[0x7F, b'E', b'L', b'F'],
            "Linux: expected ELF magic bytes"
        );
        println!("ELF magic verified");
    }

    #[cfg(target_os = "macos")]
    {
        // Mach-O 64-bit magic: 0xCF 0xFA 0xED 0xFE (little-endian) or
        //                      0xFE 0xED 0xFA 0xCF (big-endian)
        let magic = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        assert!(
            magic == 0xFEED_FACF || magic == 0xCEFA_EDFE || magic == 0xFEED_FACE
                || magic == 0xCEFA_EDCE,
            "macOS: expected Mach-O magic, got {magic:#010x}"
        );
        println!("Mach-O magic verified: {magic:#010x}");
    }

    #[cfg(target_os = "windows")]
    {
        // Cranelift's ObjectModule on Windows emits COFF (not PE).
        // COFF x86-64 machine type: 0x8664 stored as LE bytes [0x64, 0x86].
        // Some toolchains may also emit a PE/COFF with DOS stub (MZ: 0x4D 0x5A).
        let is_coff = bytes[0] == 0x64 && bytes[1] == 0x86;
        let is_pe   = bytes[0] == 0x4D && bytes[1] == 0x5A; // "MZ"
        assert!(
            is_coff || is_pe,
            "Windows: expected COFF (0x64 0x86) or PE (MZ) header, \
             got {:#04x} {:#04x}",
            bytes[0],
            bytes[1]
        );
        if is_coff {
            println!("COFF x86-64 magic verified");
        } else {
            println!("PE/COFF magic (MZ) verified");
        }
    }
}
