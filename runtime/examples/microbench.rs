use nagi_runtime as rt;
use rt::{
    metrics::{benchmark, measure},
    rusqlite::{Connection, Row},
    FromRow,
};
use serde::{Deserialize, Serialize};
use std::hint::black_box;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct User {
    id: i64,
    name: String,
    age: i32,
}
impl FromRow for User {
    fn columns() -> &'static [&'static str] {
        &["id", "name", "age"]
    }
    fn read(r: &Row<'_>, ix: &[usize]) -> rt::rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(ix[0])?,
            name: r.get(ix[1])?,
            age: r.get(ix[2])?,
        })
    }
}
fn sum(x: &[i64]) -> i64 {
    let mut s = 0;
    for &v in x {
        s += v;
    }
    s
}
fn map(x: &[i64]) -> i64 {
    let mut s = 0;
    for &v in x {
        s += v * 3 + 1;
    }
    s
}
fn filter(x: &[i64]) -> i64 {
    let mut s = 0;
    for &v in x {
        if v > 0 {
            s += v;
        }
    }
    s
}
fn branch(x: &[i64]) -> i64 {
    let mut s = 0;
    for &v in x {
        if v % 7 == 0 {
            s += v;
        } else {
            s -= v;
        }
    }
    s
}
fn floats(x: &[f64]) -> f64 {
    let mut s = 0.0;
    for &v in x {
        s += v;
    }
    s
}
fn loops(n: i64) -> i64 {
    let mut s = 0;
    for i in 0..n {
        s += i % 7;
    }
    s
}

fn main() {
    rt::bench_i64("rust_integer_sum", 100000, sum);
    rt::bench_i64("rust_map_reduce", 100000, map);
    rt::bench_i64("rust_filter_reduce", 100000, filter);
    rt::bench_i64("rust_branch", 100000, branch);
    rt::bench_f64("rust_float_sum", 100000, floats);
    rt::bench_scalar("rust_loop", 100000, loops);
    let input = br#"{"id":42,"name":"tp-li","age":18}"#;
    let user: User = rt::decode(input).unwrap();
    benchmark("json_parse_tree", 1, || {
        black_box(rt::decode::<serde_json::Value>(black_box(input)).unwrap())
    });
    benchmark("json_tree_to_class", 1, || {
        let tree: serde_json::Value = rt::decode(black_box(input)).unwrap();
        black_box(serde_json::from_value::<User>(tree).unwrap())
    });
    benchmark("json_direct_class", 1, || {
        black_box(rt::decode::<User>(black_box(input)).unwrap())
    });
    benchmark("json_class_encode", 1, || {
        black_box(rt::encode(black_box(&user)).unwrap())
    });
    benchmark("json_parse_modify_encode", 1, || {
        let mut u: User = rt::decode(black_box(input)).unwrap();
        u.age += 1;
        black_box(rt::encode(&u).unwrap())
    });
    let mut buffer = Vec::with_capacity(256);
    benchmark("json_encode_reused_buffer", 1, || {
        buffer.clear();
        serde_json::to_writer(&mut buffer, black_box(&user)).unwrap();
        black_box(buffer.len())
    });
    #[derive(Deserialize)]
    struct Borrow<'a> {
        id: i64,
        name: &'a str,
        age: i32,
    }
    benchmark("json_borrowed_class", 1, || {
        let u: Borrow = rt::decode(black_box(input)).unwrap();
        black_box((u.id, u.name, u.age))
    });
    let names: Vec<String> = (0..1000).map(|i| format!("{}", i * 17)).collect();
    benchmark("string_parse_i64_1000", 1000, || {
        let mut n = 0;
        for s in &names {
            n += rt::parse_i64(black_box(s)).unwrap();
        }
        black_box(n)
    });
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE users(id INTEGER PRIMARY KEY,name TEXT,age INTEGER)")
        .unwrap();
    {
        let tx = conn.transaction().unwrap();
        {
            let mut stmt = tx.prepare("INSERT INTO users VALUES (?1,?2,?3)").unwrap();
            for i in 1..=10000 {
                stmt.execute(rt::rusqlite::params![i, "tp-li", 18]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    for n in [1, 100, 1000, 10000] {
        benchmark(&format!("db_named_{n}"), n, || {
            let mut s = conn
                .prepare_cached("SELECT id,name,age FROM users WHERE id<=?1")
                .unwrap();
            let mut rows = s.query([n as i64]).unwrap();
            let mut out = vec![];
            while let Some(r) = rows.next().unwrap() {
                out.push(User {
                    id: r.get("id").unwrap(),
                    name: r.get("name").unwrap(),
                    age: r.get("age").unwrap(),
                });
            }
            black_box(out)
        });
        benchmark(&format!("db_indexed_{n}"), n, || {
            let mut s = conn
                .prepare_cached("SELECT id,name,age FROM users WHERE id<=?1")
                .unwrap();
            let ix = rt::database_indices::<User>(&s).unwrap();
            let mut rows = s.query([n as i64]).unwrap();
            let mut out = vec![];
            while let Some(r) = rows.next().unwrap() {
                out.push(User::read(r, &ix).unwrap());
            }
            black_box(out)
        });
        benchmark(&format!("db_reserved_{n}"), n, || {
            let mut s = conn
                .prepare_cached("SELECT id,name,age FROM users WHERE id<=?1")
                .unwrap();
            let ix = rt::database_indices::<User>(&s).unwrap();
            let mut rows = s.query([n as i64]).unwrap();
            let mut out = Vec::with_capacity(n);
            while let Some(r) = rows.next().unwrap() {
                out.push(User::read(r, &ix).unwrap());
            }
            black_box(out)
        });
        benchmark(&format!("db_step_id_{n}"), n, || {
            let mut s = conn
                .prepare_cached("SELECT id,name,age FROM users WHERE id<=?1")
                .unwrap();
            let mut rows = s.query([n as i64]).unwrap();
            let mut out = 0i64;
            while let Some(r) = rows.next().unwrap() {
                out += r.get::<_, i64>(0).unwrap();
            }
            black_box(out)
        });
    }
    benchmark("db_insert_transaction_1000", 1000, || {
        let tx = conn.transaction().unwrap();
        {
            let mut s = tx.prepare("INSERT INTO users VALUES(?1,?2,?3)").unwrap();
            for i in 10001..11001 {
                s.execute(rt::rusqlite::params![i, "tp-li", 18]).unwrap();
            }
        }
        tx.rollback().unwrap();
    });
    benchmark("db_single_insert_rollback", 1, || {
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO users VALUES(?1,?2,?3)",
            rt::rusqlite::params![10001, "tp-li", 18],
        )
        .unwrap();
        tx.rollback().unwrap();
    });
    let original = vec![7u8; 1024 * 1024];
    benchmark("view_1mib", 1, || {
        black_box(rt::slice(black_box(&original), 1, 1024 * 1024).unwrap())
    });
    benchmark("copy_1mib", 1, || {
        black_box(
            rt::slice(black_box(&original), 1, 1024 * 1024)
                .unwrap()
                .to_owned(),
        )
    });
    let (view, m) = measure(|| rt::slice(&original, 1, 100).unwrap());
    println!(
        "{}",
        serde_json::json!({"name":"zero_copy_proof","allocation":m,"input_address":original.as_ptr() as usize,"view_address":view.as_ptr() as usize,"offset":view.as_ptr() as usize-original.as_ptr() as usize})
    );
    #[derive(Clone, Copy)]
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    let points = vec![Point { x: 1.0, y: 2.0 }; 100];
    println!(
        "{}",
        serde_json::json!({"name":"class_layout_proof","point_size":std::mem::size_of::<Point>(),"stride":&points[1] as *const Point as usize-&points[0] as *const Point as usize,"checksum":points[0].x+points[0].y})
    );
}
