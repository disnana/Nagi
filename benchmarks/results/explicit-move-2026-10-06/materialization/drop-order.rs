use std::sync::Mutex;
static EVENTS:Mutex<Vec<i64>>=Mutex::new(Vec::new());
struct Marker(i64);
impl Drop for Marker {fn drop(&mut self){EVENTS.lock().unwrap().push(200+self.0);}}
fn marker(id:i64)->Marker {EVENTS.lock().unwrap().push(100+id);Marker(id)}
fn history()->Vec<i64>{std::mem::take(&mut *EVENTS.lock().unwrap())}
fn paren(){let values=vec![marker(1)];let _n=(values).len();EVENTS.lock().unwrap().push(300);}
fn identity(){let values=vec![marker(1)];let _n=std::convert::identity(values).len();EVENTS.lock().unwrap().push(300);}
fn rhs(){let mut old=marker(2);let source=marker(3);old=std::convert::identity(source);EVENTS.lock().unwrap().push(300);}
fn consume_view(value:&str){assert_eq!(value,"Nagi");}
fn view(){consume_view(std::convert::identity(String::from("Nagi").as_str()));consume_view(std::convert::identity(std::convert::identity(String::from("Nagi").as_str())));}
fn main(){paren();println!("paren={:?}",history());identity();println!("identity={:?}",history());rhs();println!("rhs={:?}",history());view();}
