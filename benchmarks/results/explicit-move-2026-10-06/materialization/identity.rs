use std::sync::Mutex;
static EVENTS: Mutex<Vec<i64>>=Mutex::new(Vec::new());
struct Marker;
impl Drop for Marker { fn drop(&mut self) { EVENTS.lock().unwrap().push(1); } }
struct Pair { number: Option<i64>, text: String }
fn take(pair: Pair)->Option<i64> {pair.number}
fn read(value: &str) {assert_eq!(value,"Nagi");}
fn main(){ let values=vec![Marker]; let count=std::convert::identity(values).len(); assert_eq!(count,1); EVENTS.lock().unwrap().push(2); println!("{:?}", *EVENTS.lock().unwrap()); read(std::convert::identity(String::from("Nagi").as_str())); let pair=Pair{number:Some(3),text:String::from("owner")}; assert!(std::convert::identity(pair.number) == take(pair)); }
