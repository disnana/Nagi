use std::{path::Path, time::Instant};
use nagic::ast::{ModuleId, Program};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let start = Instant::now();
    let mut sources = nagic::source::load(Path::new(&path), !path.ends_with(".low")).unwrap();
    let load = start.elapsed();
    let mut primary = std::mem::take(&mut sources.program);
    let mut native = Program::default();
    nagic::modules::rebind_native(&mut primary, &mut native).unwrap();
    let start = Instant::now();
    let value = nagic::symbols::index(&sources, &[&primary, &native]).unwrap();
    let index = start.elapsed();
    let start = Instant::now();
    let text = value.to_string();
    let json = start.elapsed();
    println!("load_us={} index_us={} json_us={} bytes={}", load.as_micros(), index.as_micros(), json.as_micros(), text.len());
    for &module in nagic::stdlib::MODULES {
        let info = nagic::stdlib::module_info(module);
        let id = ModuleId(info.id.into());
        let start = Instant::now();
        let definitions = nagic::stdlib::definitions(&id);
        println!("module={} definitions={} definitions_us={}", info.name, definitions.len(), start.elapsed().as_micros());
    }
}
