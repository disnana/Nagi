#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_496e6c696e65 {
    pub value: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_496e6c696e65 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Inline").field("value", &self.value).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_496e6c696e65 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["value"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
value: row.get(ix[0])?,
}) }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_496e6c696e65 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.value)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4f776e6564 {
    pub label: ::std::string::String,
    pub values: ::std::vec::Vec<__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_496e6c696e65>,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4f776e6564 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Owned").field("label", &self.label).field("values", &self.values).finish()
    }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4f776e6564 {
    const INLINE_ONLY: ::std::primitive::bool = false;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.label)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        let __nagi_charge_field_1 = walk.visit(&self.values)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_1)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4e6f6465 {
    pub children: ::std::vec::Vec<__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4e6f6465>,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4e6f6465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Node").field("children", &self.children).finish()
    }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4e6f6465 {
    const INLINE_ONLY: ::std::primitive::bool = false;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.children)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_436175736564 {
    pub cause: ::nagi_runtime::Error,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_436175736564 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Caused").field("cause", &self.cause).finish()
    }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_436175736564 {
    const INLINE_ONLY: ::std::primitive::bool = false;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.cause)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug)]
pub enum __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_e_5061636b6574 {
    Empty,
    Number {
        value: ::std::primitive::i64,
    },
    Data {
        value: __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4f776e6564,
    },
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_e_5061636b6574 {
    const INLINE_ONLY: ::std::primitive::bool = false;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        match self {
            Self::Empty => ::std::result::Result::Ok(0),
            Self::Number { value: __nagi_charge_value_0 } => {
                let mut __nagi_charge_heap: ::std::primitive::usize = 0;
                let __nagi_charge_field_0 = walk.visit(__nagi_charge_value_0)?;
                __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
                ::std::result::Result::Ok(__nagi_charge_heap)
            },
            Self::Data { value: __nagi_charge_value_0 } => {
                let mut __nagi_charge_heap: ::std::primitive::usize = 0;
                let __nagi_charge_field_0 = walk.visit(__nagi_charge_value_0)?;
                __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
                ::std::result::Result::Ok(__nagi_charge_heap)
            },
        }
    }
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_496e6c696e65 as Inline;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4f776e6564 as Owned;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_4e6f6465 as Node;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_c_436175736564 as Caused;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f6163746f722d646174612f6d61696e2e6e616769_e_5061636b6574 as Packet;
#[allow(non_snake_case)]
pub mod actor {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Supervisor as Supervisor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Control as Control;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Actor as Actor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Turn as Turn;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Options as Options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::ActorOptions as ActorOptions;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::RestartPolicy as RestartPolicy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::CallKind as CallKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::EventKind as EventKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::CallError as CallError;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Event as Event;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::TaskReady as TaskReady;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::WaitKind as WaitKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::WaitError as WaitError;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::default_options as default_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::options as options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::default_actor_options as default_actor_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::actor_options as actor_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::restart_delay as restart_delay;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::supervisor as supervisor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::control as control;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::clone_control as clone_control;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::register as register;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::task as task;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::turn as turn;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::clone_actor as clone_actor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::ready as ready;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::call as call;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::run as run;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::shutdown as shutdown;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::next_event as next_event;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::next_event_timeout as next_event_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::task_with_ready as task_with_ready;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::mark_ready as mark_ready;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::yield_now as yield_now;
}
fn main() {}
