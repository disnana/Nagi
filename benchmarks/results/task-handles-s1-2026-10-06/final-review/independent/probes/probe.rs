
use nagic::{check,source,emit};
use std::path::Path;
fn main(){let args:Vec<_>=std::env::args().collect(); if args.len()==3 {let file=Path::new(&args[1]);let mut l=source::load(file,file.extension().unwrap()=="nagi").unwrap();check::check(&mut l.program).unwrap();let low=emit::low(&l.program);let saved=file.with_extension("saved.low");std::fs::write(&saved,low).unwrap();let l=source::load(&saved,false).unwrap();let sealed=check::finalize(l.program,Default::default(),source::SourceProvenance::user_low_unmapped()).unwrap();std::fs::write(&args[2],emit::rust(&sealed).unwrap()).unwrap();return;} let root=Path::new("/tmp/nagi-s1-final-review/probes");
let header="from std.task import discard\nfrom std.ownership import move\nasync def work() -> i64:\n    return 7\nasync def exercise(flag: bool) -> Result[unit, Error]:\n    async with scope:\n";
let cases=[
("nested-move-receive",true,"        task = spawn work()\n        received = await move(move(task))\n"),
("self-move-alias",true,"        task = spawn work()\n        task = move(task)\n        received = await task\n"),
("nested-move-discard",true,"        task = spawn work()\n        discard(move(move(task)))\n"),
("branch-alias-transfer",true,"        task = spawn work()\n        if flag:\n            alias = move(task)\n            discard(alias)\n        else:\n            received = await task\n"),
("shadow-same-type-inner",false,"        task = spawn work()\n        async with scope:\n            task = spawn work()\n            discard(task)\n        discard(task)\n"),
("outer-consumed-new-inner",false,"        task = spawn work()\n        discard(task)\n        async with scope:\n            task = spawn work()\n            discard(task)\n"),
("unreceived-after-conditional-try",false,"        task = spawn work()\n        if flag:\n            outcome: Result[unit,Error] = error(\"conditional error\")\n            result = try outcome\n"),
("for-branch-local-obligation",false,"        for i in range(2):\n            if flag:\n                task = spawn work()\n            else:\n                print(0)\n"),
("same-name-user-discard",true,"        task = spawn work()\n        received = await task\n        discard(1)\n"),
];
for (name,wanted,body) in cases {let extra=if name=="same-name-user-discard"{"def discard(value: i64) -> unit:\n    print(value)\n"}else{""};
 let mut high=header.to_string();if !extra.is_empty(){high=high.replace("from std.task import discard\n","");high=format!("{extra}{high}");}high+=body;high+="    return ok(print(0))\n";
 let file=root.join(format!("{name}.nagi"));std::fs::write(&file,&high).unwrap();
 let outcome=source::load(&file,true).and_then(|mut l|{check::check(&mut l.program)?;Ok(l.program)});
 match outcome {Err(e)=>println!("{name}: phase=check accepted=false expected={wanted} diagnostic={e}"),Ok(program)=>{
  let low=emit::low(&program);let saved=root.join(format!("{name}.low"));std::fs::write(&saved,low).unwrap();
  let l=source::load(&saved,false).unwrap();let sealed=check::finalize(l.program,Default::default(),source::SourceProvenance::user_low_unmapped()).unwrap();
  let rust=emit::rust(&sealed).unwrap();std::fs::write(root.join(format!("{name}.rs")),rust).unwrap();
  println!("{name}: phase=emit accepted=true expected={wanted}");
 }}
}
}
