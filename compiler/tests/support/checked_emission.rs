//! Existing in-memory fixtures still compare their canonical High/Low ASTs.
//! Seal each fixture explicitly; production emission has no Program overload.
pub fn seal(program: &nagic::ast::Program) -> nagic::checked::CheckedProgram {
    nagic::check::finalize(
        program.clone(),
        nagic::ast::Program::default(),
        nagic::source::SourceProvenance::user_low_unmapped(),
    )
    .unwrap_or_else(|error| panic!("fixture finalization failed: {error}"))
}
