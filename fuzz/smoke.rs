// coverage-guided fuzzではなく、固定seedのmutation smoke。失敗入力を再現可能にする。
use nagic::{check, parser};
fn main() {
    let corpus=["def main():\n    print(\"hello\")\n","class U:\n    id: i64\ndef main():\n    u = U(id=1)\n","fn main() -> unit { let x: i64 = 1; print(x); }","async def main() -> Result[unit,Error]:\n    async with scope:\n        spawn sleep(1)\n    return ok(print(1))\n"];
    let mut seed = 0x12345678u64;
    let mut checked = 0;
    for i in 0..10000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let source = corpus[i % corpus.len()];
        let mut bytes = source.as_bytes().to_vec();
        let at = seed as usize % bytes.len();
        let edit = (seed >> 32) as u8;
        if i % 3 == 0 {
            bytes.remove(at);
        } else if i % 3 == 1 {
            bytes.insert(at, edit);
        } else {
            bytes[at] = edit;
        }
        let s = String::from_utf8_lossy(&bytes);
        let result = std::panic::catch_unwind(|| {
            if let Ok(mut p) = parser::parse(&s, i % corpus.len() != 2) {
                let _ = check::check(&mut p);
            }
        });
        if result.is_err() {
            eprintln!("panic seed={seed} input={s:?}");
            std::process::exit(1);
        }
        checked += 1;
    }
    for i in 0..10000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut data = br#"{"id":42,"name":"alice","age":18}"#.to_vec();
        let at = seed as usize % data.len();
        data[at] = (seed >> 32) as u8;
        let r = std::panic::catch_unwind(|| {
            let _ = serde_json::from_slice::<serde_json::Value>(&data);
        });
        assert!(r.is_ok(), "JSON panic {i}");
    }
    println!("{{\"parser_mutations\":{checked},\"json_mutations\":10000,\"seed\":305419896,\"panics\":0}}");
}
