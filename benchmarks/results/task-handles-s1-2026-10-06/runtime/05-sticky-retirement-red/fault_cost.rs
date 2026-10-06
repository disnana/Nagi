
use nagi_runtime::{TaskScope,Error};
use std::time::Instant;
async fn trial(n:usize,receive:bool)->u128 {
 let mut s=TaskScope::new();
 let tasks:Vec<_>=(0..n).map(|i|s.spawn_value(async move{i})).collect();
 s.spawn(async{Err(Error::invalid("sticky"))});
 // Fault may abort tasks; output retention is irrelevant to receipt LIVE records.
 assert!(s.join().await.is_err());
 let start=Instant::now();
 for t in tasks { if receive {assert!(s.receive(t).await.is_err());} else {s.discard(t);} }
 assert!(s.join().await.is_err());
 start.elapsed().as_nanos()
}
fn main(){let rt=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
 for n in [128,1024,8192] {for receive in [true,false] {let mut samples=vec![];
  for _ in 0..3 { samples.push(rt.block_on(trial(n,receive))); }
  println!("items={n} mode={} raw_ns={samples:?}",if receive{"sticky_receive"}else{"sticky_discard"});
 }}
}
