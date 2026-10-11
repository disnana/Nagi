use super::native_triple::Fixture;

#[test]
fn typed_monitor_preserves_service_errors_shutdown_and_parent_drop() {
    Fixture::new().run_three("task-service-s2", HIGH, LOW, ADAPTER, ASSERTIONS);
}

const HIGH: &str = r#"from std.task import discard, kind, message, TaskFailureKind
@rust("native::monitor")
extern async def monitor() -> Result[unit, Error]
@rust("native::http")
extern async def http() -> Result[unit, Error]
@rust("native::fault_seen")
extern def fault_seen(value: TaskFailureKind) -> unit
async def typed_monitor_service() -> Result[unit, Error]:
    async with scope:
        monitor_task = spawn monitor()
        spawn http()
        received = await monitor_task
        match received:
            case Ok(inner):
                try inner
            case Err(failure):
                fault_seen(kind(failure))
                print(message(failure))
    return ok(assert_true(True))
async def discarded_monitor_service() -> Result[unit, Error]:
    async with scope:
        monitor_task = spawn monitor()
        discard(monitor_task)
        spawn http()
    return ok(assert_true(True))
"#;

const LOW: &str = r#"from std.task import discard, kind, message, TaskFailureKind;
@rust("native::monitor")
extern async fn monitor() -> Result[unit, Error];
@rust("native::http")
extern async fn http() -> Result[unit, Error];
@rust("native::fault_seen")
extern fn fault_seen(value: TaskFailureKind) -> unit;
async fn typed_monitor_service() -> Result[unit, Error] {
    scope {
        let monitor_task = spawn monitor();
        spawn http();
        let received = await monitor_task;
        match received {
            case Ok(inner) { try inner; }
            case Err(failure) { fault_seen(kind(failure)); print(message(failure)); }
        }
    }
    return ok(assert_true(True));
}
async fn discarded_monitor_service() -> Result[unit, Error] {
    scope {
        let monitor_task = spawn monitor();
        discard(monitor_task);
        spawn http();
    }
    return ok(assert_true(True));
}
"#;

const ADAPTER: &str = r#"
mod native {
    use nagi_runtime::{actor, http_server as web, Error, ErrorKind, TaskFailureKind};
    use std::{future::{poll_fn, Future}, net::SocketAddr, pin::Pin,
        sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}, task::Poll};
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::{Notify, oneshot}};

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Case { Normal, Terminal, Discarded, HttpError, MonitorPanic, ParentDrop, RetainedContext }
    pub struct Events {
        pub monitor_returned: AtomicBool,
        pub monitor_error: Mutex<Option<(ErrorKind, String)>>,
        pub http_returned: AtomicBool,
        pub http_dropped: AtomicBool,
        pub http_signal_sent: AtomicBool,
        pub handler_started: AtomicBool,
        pub handler_dropped: AtomicBool,
        pub context_dropped: AtomicBool,
        pub fault: AtomicUsize,
        pub drop_entered: AtomicBool,
        changed: Notify,
        gate: Mutex<bool>,
        gate_changed: Condvar,
        case: Case,
    }
    struct Context(Arc<Events>);
    impl Drop for Context {
        fn drop(&mut self) { self.0.context_dropped.store(true, Ordering::Release); self.0.changed.notify_waiters(); }
    }
    struct Service {
        group: Option<actor::Supervisor<Context>>,
        held_context: Option<Arc<Context>>,
        listener: Option<TcpListener>,
        counter: actor::Actor<i64, i64, String>,
        control: actor::Control,
        shutdown: Option<oneshot::Sender<()>>,
        stopped: Option<oneshot::Receiver<()>>,
        address: SocketAddr,
        events: Arc<Events>,
    }
    static SERVICE: Mutex<Option<Service>> = Mutex::new(None);
    struct HttpGuard(Arc<Events>);
    impl Drop for HttpGuard {
        fn drop(&mut self) {
            if self.0.case == Case::ParentDrop {
                self.0.drop_entered.store(true, Ordering::Release);
                self.0.changed.notify_waiters();
                let gate = self.0.gate.lock().unwrap();
                let (_gate, expired) = self.0.gate_changed.wait_timeout_while(gate,
                    std::time::Duration::from_secs(10), |open| !*open).unwrap();
                assert!(!expired.timed_out(), "finite HTTP Drop gate not released");
            }
            self.0.http_dropped.store(true, Ordering::Release);
            self.0.changed.notify_waiters();
        }
    }
    struct HandlerGuard(Arc<Events>);
    impl Drop for HandlerGuard {
        fn drop(&mut self) { self.0.handler_dropped.store(true, Ordering::Release); self.0.changed.notify_waiters(); }
    }
    pub struct ReleaseOnDrop(pub Arc<Events>);
    impl Drop for ReleaseOnDrop { fn drop(&mut self) { release_drop_gate(&self.0); } }
    pub fn release_drop_gate(events: &Events) {
        *events.gate.lock().unwrap() = true;
        events.gate_changed.notify_all();
    }
    pub async fn wait(events: &Events, flag: &AtomicBool) {
        loop {
            let changed = events.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if flag.load(Ordering::Acquire) { break; }
            changed.await;
        }
    }
    pub async fn poll_pending<T>(mut future: Pin<&mut impl Future<Output=T>>) {
        poll_fn(|cx| {
            assert!(matches!(future.as_mut().poll(cx), Poll::Pending), "service unexpectedly completed");
            Poll::Ready(())
        }).await;
    }
    pub async fn setup(case: Case) -> (Arc<Events>, actor::Control) {
        let events = Arc::new(Events {
            monitor_returned: AtomicBool::new(false), monitor_error: Mutex::new(None),
            http_returned: AtomicBool::new(false), http_dropped: AtomicBool::new(false),
            http_signal_sent: AtomicBool::new(false), handler_started: AtomicBool::new(false),
            handler_dropped: AtomicBool::new(false), context_dropped: AtomicBool::new(false),
            fault: AtomicUsize::new(0), drop_entered: AtomicBool::new(false), changed: Notify::new(),
            gate: Mutex::new(false), gate_changed: Condvar::new(), case,
        });
        let group = actor::supervisor(Context(events.clone()), actor::options(1, 16, 0, 10000, 1000).unwrap());
        let counter = actor::register(&group, "counter", |context: Arc<Context>| async move {
            if context.0.case == Case::RetainedContext {
                SERVICE.lock().unwrap().as_mut().unwrap().held_context = Some(context.clone());
            }
            Ok(9_i64)
        }, |state: i64, change: i64| async move {
            if change == -2 { return Err(Error::invalid("terminal sentinel")); }
            let reply = if change < 0 { Err("business sentinel".to_owned()) } else { Ok(state) };
            Ok(actor::turn(state, reply))
        }, actor::default_actor_options()).unwrap();
        let control = actor::control(&group);
        let observer = actor::clone_control(&control);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, stopped) = oneshot::channel();
        let mut slot = SERVICE.lock().unwrap();
        assert!(slot.is_none());
        *slot = Some(Service { group: Some(group), held_context: None, listener: Some(listener),
            counter, control, shutdown: Some(shutdown), stopped: Some(stopped), address, events: events.clone() });
        (events, observer)
    }
    pub fn monitor() -> impl Future<Output=Result<(), Error>> {
        let (group, events) = {
            let mut slot=SERVICE.lock().unwrap(); let state=slot.as_mut().unwrap();
            (state.group.take().unwrap(), state.events.clone())
        };
        async move {
            let result=actor::run(group).await;
            if let Err(error)=&result { *events.monitor_error.lock().unwrap()=Some((error.kind, error.message.clone())); }
            events.monitor_returned.store(true, Ordering::Release);
            events.changed.notify_waiters();
            if events.case == Case::MonitorPanic { panic!("controlled monitor-task panic after actor cleanup"); }
            result
        }
    }
    pub fn fault_seen(kind: TaskFailureKind) {
        let value=match kind { TaskFailureKind::Panicked=>1, TaskFailureKind::Cancelled=>2,
            TaskFailureKind::LegacyError=>3, TaskFailureKind::Internal=>4 };
        SERVICE.lock().unwrap().as_ref().unwrap().events.fault.store(value, Ordering::Release);
    }
    pub async fn http() -> Result<(), Error> {
        let (listener, counter, stopped, events) = {
            let mut slot=SERVICE.lock().unwrap(); let state=slot.as_mut().unwrap();
            (state.listener.take().unwrap(), actor::clone_actor(&state.counter),
                state.stopped.take().unwrap(), state.events.clone())
        };
        let _guard=HttpGuard(events.clone());
        let app=web::route(web::app_default(counter), web::Method::GET, "/counter", web::public_policy(), |_, counter, ()| async move {
            Ok(match actor::call(&counter, 0, 0, 1000).await {
                Ok(Ok(value))=>web::text(web::Status::OK, &value.to_string()),
                _=>web::text(web::Status::SERVICE_UNAVAILABLE, "stopped"),
            })
        }).unwrap();
        let app=web::route(app, web::Method::GET, "/business", web::public_policy(), |_, counter, ()| async move {
            let reply=actor::call(&counter, -1, 0, 1000).await.unwrap();
            assert_eq!(reply, Err("business sentinel".to_owned()));
            Ok(web::text(web::Status::CONFLICT, "business sentinel"))
        }).unwrap();
        let held=events.clone();
        let app=web::route(app, web::Method::GET, "/hold", web::public_policy(), move |_, _, ()| {
            let held=held.clone(); async move {
                let _guard=HandlerGuard(held.clone());
                held.handler_started.store(true, Ordering::Release); held.changed.notify_waiters();
                std::future::pending::<()>().await;
                Ok(web::empty(web::Status::NO_CONTENT))
            }
        }).unwrap();
        let options=web::authority(web::default_options(), "https://localhost", vec!["localhost".to_owned()], 1, 128)?;
        web::serve_listener(listener, app, options, async { let _=stopped.await; }).await?;
        events.http_returned.store(true, Ordering::Release); events.changed.notify_waiters();
        if events.case == Case::HttpError { Err(Error::invalid("HTTP outer error sentinel")) } else { Ok(()) }
    }
    pub async fn connect() -> TcpStream {
        let address=SERVICE.lock().unwrap().as_ref().unwrap().address;
        TcpStream::connect(address).await.unwrap()
    }
    pub async fn request(path: &str) -> Vec<u8> {
        let mut socket=connect().await;
        socket.write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut response=Vec::new(); socket.read_to_end(&mut response).await.unwrap(); response
    }
    pub async fn business() {
        let counter=actor::clone_actor(&SERVICE.lock().unwrap().as_ref().unwrap().counter);
        actor::ready(&counter, 1000).await.unwrap();
        let response=request("/business").await;
        assert!(response.starts_with(b"HTTP/1.1 409 "), "{response:?}");
        assert!(response.ends_with(b"business sentinel"));
        let response=request("/counter").await;
        assert!(response.starts_with(b"HTTP/1.1 200 "), "{response:?}");
        assert!(response.ends_with(b"9"));
    }
    pub async fn terminal() {
        let counter=actor::clone_actor(&SERVICE.lock().unwrap().as_ref().unwrap().counter);
        assert_eq!(actor::call(&counter, -2, 0, 1000).await.unwrap_err().kind(), actor::CallKind::REPLY_LOST);
    }
    pub fn stop_http() {
        let mut slot=SERVICE.lock().unwrap(); let service=slot.as_mut().unwrap();
        if let Some(signal)=service.shutdown.take() {
            service.events.http_signal_sent.store(true, Ordering::Release);
            let _=signal.send(());
        }
    }
    pub fn release_context() {
        let held=SERVICE.lock().unwrap().as_mut().unwrap().held_context.take(); drop(held);
    }
    pub async fn closed(events: &Events) {
        wait(events, &events.http_dropped).await;
        let address=SERVICE.lock().unwrap().as_ref().unwrap().address;
        assert!(TcpStream::connect(address).await.is_err(), "HTTP listener survived scope exit");
    }
    pub fn finish() { SERVICE.lock().unwrap().take().unwrap(); }
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn typed_service_contracts() {
    use native::Case;
    use std::sync::atomic::Ordering;
    let rt=tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    rt.block_on(async {
        for case in [Case::Normal, Case::Terminal, Case::Discarded, Case::HttpError,
            Case::MonitorPanic, Case::ParentDrop, Case::RetainedContext] {
            let (events, control)=native::setup(case).await;
            let _release=native::ReleaseOnDrop(events.clone());
            let observed=tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut parent=Box::pin(if case == Case::Discarded {
                    Box::pin(discarded_monitor_service()) as std::pin::Pin<Box<dyn std::future::Future<Output=Result<(),nagi_runtime::Error>>>>
                } else { Box::pin(typed_monitor_service()) });
                native::poll_pending(parent.as_mut()).await;
                native::business().await;
                match case {
                    Case::Terminal | Case::Discarded => {
                        native::terminal().await;
                        native::wait(&events, &events.monitor_returned).await;
                        let original=events.monitor_error.lock().unwrap().clone().unwrap();
                        assert!(matches!(original.0, nagi_runtime::ErrorKind::Internal));
                        assert!(original.1.contains("restart intensity"));
                        if case == Case::Discarded {
                            // Valid Nagi, wrong service composition: abandoning inner Err
                            // keeps HTTP healthy until an independent, finite shutdown.
                            native::poll_pending(parent.as_mut()).await;
                            assert!(native::request("/counter").await.starts_with(b"HTTP/1.1 503 "));
                            assert!(!events.http_dropped.load(Ordering::Acquire));
                            native::stop_http(); parent.await.unwrap();
                        } else {
                            let error=parent.await.unwrap_err();
                            assert_eq!(std::mem::discriminant(&error.kind), std::mem::discriminant(&original.0));
                            assert_eq!(error.message, original.1);
                            assert!(!events.http_signal_sent.load(Ordering::Acquire));
                            assert_eq!(events.fault.load(Ordering::Acquire), 0, "inner monitor Err was flattened into TaskFailure");
                        }
                    }
                    Case::Normal | Case::MonitorPanic => {
                        nagi_runtime::actor::shutdown(&control).await.unwrap();
                        native::wait(&events, &events.monitor_returned).await;
                        if case == Case::Normal {
                            native::poll_pending(parent.as_mut()).await;
                            assert!(native::request("/counter").await.starts_with(b"HTTP/1.1 503 "));
                            assert!(!events.http_dropped.load(Ordering::Acquire));
                            native::stop_http(); parent.await.unwrap();
                        } else {
                            assert!(matches!(parent.await.unwrap_err().kind, nagi_runtime::ErrorKind::Internal));
                            assert_eq!(events.fault.load(Ordering::Acquire), 1);
                            assert!(!events.http_signal_sent.load(Ordering::Acquire));
                        }
                    }
                    Case::HttpError => {
                        assert!(!events.monitor_returned.load(Ordering::Acquire));
                        native::stop_http(); native::wait(&events, &events.http_returned).await;
                        let error=parent.await.unwrap_err();
                        assert!(matches!(error.kind, nagi_runtime::ErrorKind::Invalid));
                        assert_eq!(error.message, "HTTP outer error sentinel");
                        assert_eq!(events.fault.load(Ordering::Acquire), 3);
                        // A retained Control elects the drainer after canceled run.
                        nagi_runtime::actor::shutdown(&control).await.unwrap();
                    }
                    Case::ParentDrop => {
                        let mut held=native::connect().await;
                        use tokio::io::AsyncWriteExt;
                        held.write_all(b"GET /hold HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
                        native::wait(&events, &events.handler_started).await;
                        drop(parent);
                        // Synchronous parent destruction returns while child Drop
                        // is held. Observe descendants separately after release.
                        native::wait(&events, &events.drop_entered).await;
                        assert!(!events.http_dropped.load(Ordering::Acquire));
                        native::release_drop_gate(&events);
                        native::wait(&events, &events.handler_dropped).await;
                        nagi_runtime::actor::shutdown(&control).await.unwrap();
                        drop(held);
                    }
                    Case::RetainedContext => {
                        let mut shutdown=Box::pin(nagi_runtime::actor::shutdown(&control));
                        native::poll_pending(shutdown.as_mut()).await;
                        loop {
                            let event=nagi_runtime::actor::next_event_timeout(&control, 1000).await.unwrap().unwrap();
                            if event.kind() == nagi_runtime::actor::EventKind::STOPPED { break; }
                        }
                        native::poll_pending(parent.as_mut()).await;
                        assert!(!events.monitor_returned.load(Ordering::Acquire));
                        assert!(!events.context_dropped.load(Ordering::Acquire));
                        assert!(native::request("/counter").await.starts_with(b"HTTP/1.1 503 "));
                        native::release_context(); shutdown.await.unwrap();
                        native::wait(&events, &events.monitor_returned).await;
                        native::poll_pending(parent.as_mut()).await;
                        native::stop_http(); parent.await.unwrap();
                    }
                }
                if case != Case::ParentDrop {
                    assert!(events.http_dropped.load(Ordering::Acquire),
                        "parent returned before its direct HTTP child was dropped: {case:?}");
                }
                native::closed(&events).await;
                assert!(events.context_dropped.load(Ordering::Acquire));
            }).await;
            // Watchdog failures release native gates, request both services to
            // stop and drain actor ownership before reporting the failed case.
            native::release_drop_gate(&events); native::release_context(); native::stop_http();
            let _=nagi_runtime::actor::shutdown(&control).await;
            native::finish();
            observed.unwrap_or_else(|_| panic!("S2 watchdog: {case:?}"));
            println!("S2 scenario: {case:?} passed");
        }
    });
}
"#;
