// Build script: generates the cucumber step bindings of the cell's own BDD
// suite (tests/bdd), from the canonical language (bdd/grammar.yaml) and the
// cell's steps on top of it (bdd/grammar.yaml in this crate).
//
// Recording a chronicle is the cell's job, and only the Rust engine plays one
// forward, so the cell's steps stay out of the canonical grammar and the JS
// engines never see them. The generator is the engine's own
// (`packages/engine/build_codegen`), included here, so there is one
// generator. Like the engine's build script, it writes an empty stub
// when the canonical grammar is absent (a build context that copies only
// `packages/`); only the `bdd` test target uses the bindings.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

include!("../engine/build_codegen/grammar_model.rs");
include!("../engine/build_codegen/gen_rust.rs");

fn main() {
    let base = Path::new("../../bdd/grammar.yaml");
    let extension = Path::new("bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../engine/build_codegen/grammar_model.rs");
    println!("cargo:rerun-if-changed=../engine/build_codegen/gen_rust.rs");

    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("bdd_generated_steps.rs");
    let code = if base.exists() {
        emit_rust(&load_grammar_extended(base, extension))
    } else {
        "// @generated: bdd/grammar.yaml absent — no steps.\n".to_string()
    };
    std::fs::write(&dest, code).expect("write generated steps");
}
