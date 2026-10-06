fn main() {
    for (name, receive) in [("unreceived", ""), ("received", "        received = await task\n")] {
        let source = format!("async def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n{receive}    return ok(print(0))\n");
        let mut program = nagic::parser::parse(&source, true).unwrap();
        let checked = nagic::check::check(&mut program);
        println!("{name}: raw High check = {checked:?}");
        if checked.is_ok() {
            let low = nagic::emit::low(&program);
            std::fs::write(format!("/tmp/nagi-task-review/raw-{name}.low"), &low).unwrap();
            let program = nagic::parser::parse(&low, false).unwrap();
            let final_check = nagic::check::finalize(program, nagic::ast::Program::default(), nagic::source::SourceProvenance::user_low_unmapped());
            println!("{name}: generated Low finalize = {:?}", final_check.err());
        }
    }
}
