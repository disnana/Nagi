use nagic::{check, emit, source};
use std::{env,fs,path::Path};
fn main() {
    for project in ["task-results", "supervised-service"] {
        let root=Path::new("/workspace/Nagi/test-nagi-code/library-examples").join(project);
        let mut texts=Vec::new();
        for form in ["main.nagi", "main.low"] {
            let mut loaded=source::load(&root.join(form), form.ends_with(".nagi")).unwrap();
            check::check(&mut loaded.program).unwrap();
            texts.push(emit::low(&loaded.program));
        }
        let equal=texts[0]==texts[1];
        println!("{project}: checked normalized High/Low equality = {equal}");
        fs::write(Path::new(&env::args().nth(1).unwrap()).join(format!("{project}-from-high.low")),&texts[0]).unwrap();
        fs::write(Path::new(&env::args().nth(1).unwrap()).join(format!("{project}-from-handwritten.low")),&texts[1]).unwrap();

    }
}
