#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874 {
    pub initial: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Context").field("initial", &self.initial).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["initial"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
initial: row.get(ix[0])?,
}) }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.initial)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 {
    pub completed: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("WorkerState").field("completed", &self.completed).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["completed"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
completed: row.get(ix[0])?,
}) }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.completed)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 {
    pub packing: ::std::primitive::i64,
    pub audit: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Totals").field("packing", &self.packing).field("audit", &self.audit).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["packing","audit"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
packing: row.get(ix[0])?,
audit: row.get(ix[1])?,
}) }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.packing)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        let __nagi_charge_field_1 = walk.visit(&self.audit)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_1)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 {
    pub packing_total: ::std::primitive::i64,
    pub audit_total: ::std::primitive::i64,
    pub packing_starts: ::std::primitive::i64,
    pub audit_starts: ::std::primitive::i64,
    pub connector_starts: ::std::primitive::i64,
    pub failed_events: ::std::primitive::i64,
    pub panicked_events: ::std::primitive::i64,
    pub restarts: ::std::primitive::i64,
    pub stopped_children: ::std::primitive::i64,
    pub shutdown_events: ::std::primitive::i64,
    pub connector_active: ::std::primitive::i64,
    pub connector_cleanups: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Report").field("packing_total", &self.packing_total).field("audit_total", &self.audit_total).field("packing_starts", &self.packing_starts).field("audit_starts", &self.audit_starts).field("connector_starts", &self.connector_starts).field("failed_events", &self.failed_events).field("panicked_events", &self.panicked_events).field("restarts", &self.restarts).field("stopped_children", &self.stopped_children).field("shutdown_events", &self.shutdown_events).field("connector_active", &self.connector_active).field("connector_cleanups", &self.connector_cleanups).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["packing_total","audit_total","packing_starts","audit_starts","connector_starts","failed_events","panicked_events","restarts","stopped_children","shutdown_events","connector_active","connector_cleanups"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
packing_total: row.get(ix[0])?,
audit_total: row.get(ix[1])?,
packing_starts: row.get(ix[2])?,
audit_starts: row.get(ix[3])?,
connector_starts: row.get(ix[4])?,
failed_events: row.get(ix[5])?,
panicked_events: row.get(ix[6])?,
restarts: row.get(ix[7])?,
stopped_children: row.get(ix[8])?,
shutdown_events: row.get(ix[9])?,
connector_active: row.get(ix[10])?,
connector_cleanups: row.get(ix[11])?,
}) }
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        let mut __nagi_charge_heap: ::std::primitive::usize = 0;
        let __nagi_charge_field_0 = walk.visit(&self.packing_total)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
        let __nagi_charge_field_1 = walk.visit(&self.audit_total)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_1)?;
        let __nagi_charge_field_2 = walk.visit(&self.packing_starts)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_2)?;
        let __nagi_charge_field_3 = walk.visit(&self.audit_starts)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_3)?;
        let __nagi_charge_field_4 = walk.visit(&self.connector_starts)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_4)?;
        let __nagi_charge_field_5 = walk.visit(&self.failed_events)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_5)?;
        let __nagi_charge_field_6 = walk.visit(&self.panicked_events)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_6)?;
        let __nagi_charge_field_7 = walk.visit(&self.restarts)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_7)?;
        let __nagi_charge_field_8 = walk.visit(&self.stopped_children)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_8)?;
        let __nagi_charge_field_9 = walk.visit(&self.shutdown_events)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_9)?;
        let __nagi_charge_field_10 = walk.visit(&self.connector_active)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_10)?;
        let __nagi_charge_field_11 = walk.visit(&self.connector_cleanups)?;
        __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_11)?;
        ::std::result::Result::Ok(__nagi_charge_heap)
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub enum __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62 {
    Process {
        units: ::std::primitive::i64,
    },
    Read,
    FailWorker,
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        match self {
            Self::Process { units: __nagi_charge_value_0 } => {
                let mut __nagi_charge_heap: ::std::primitive::usize = 0;
                let __nagi_charge_field_0 = walk.visit(__nagi_charge_value_0)?;
                __nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_0)?;
                ::std::result::Result::Ok(__nagi_charge_heap)
            },
            Self::Read => ::std::result::Result::Ok(0),
            Self::FailWorker => ::std::result::Result::Ok(0),
        }
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub enum __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72 {
    InvalidUnits,
}
impl ::nagi_runtime::actor::ChargeOwned for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72 {
    const INLINE_ONLY: ::std::primitive::bool = true;
    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {
        match self {
            Self::InvalidUnits => ::std::result::Result::Ok(0),
        }
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f72(mut signal: ::nagi_runtime::actor::TaskReady) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    native::connector(signal).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f737461727473() -> ::std::primitive::i64 {
    native::connector_starts()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f616374697665() -> ::std::primitive::i64 {
    native::connector_active()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f636c65616e757073() -> ::std::primitive::i64 {
    native::connector_cleanups()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6372656174655f776f726b6572(mut context: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874>) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465, ::nagi_runtime::Error> {
    return ::std::result::Result::Ok(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 { completed: (context).initial });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_70726f636573735f6a6f62(mut state: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465, mut job: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62) -> ::std::result::Result<::nagi_runtime::actor::Turn<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>, ::nagi_runtime::Error> {
    match job {
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Process { mut units } => {
            if ((units < 1i64) || (units > 100i64)) {
                return ::std::result::Result::Ok(::nagi_runtime::actor::turn::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>(state, ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72::InvalidUnits)));
            }
            let mut completed: ::std::primitive::i64 = ((state).completed + units);
            return ::std::result::Result::Ok(::nagi_runtime::actor::turn::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 { completed: completed }, ::std::result::Result::Ok(completed)));
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Read => {
            let mut completed: ::std::primitive::i64 = (state).completed;
            return ::std::result::Result::Ok(::nagi_runtime::actor::turn::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>(state, ::std::result::Result::Ok(completed)));
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::FailWorker => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("controlled packing worker failure")));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72756e5f636f6e6e6563746f72(mut context: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874>, mut signal: ::nagi_runtime::actor::TaskReady) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    return (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f72(signal)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6d6f6e69746f72(mut group: ::nagi_runtime::actor::Supervisor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874>) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    return (::nagi_runtime::actor::run(group)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374<'a>(mut worker: &'a ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>, mut job: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut worker: &::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72> = worker;
    match (::nagi_runtime::actor::call(worker, job, 5000i64, 5000i64)).await {
        ::std::result::Result::Ok(mut reply) => {
            match reply {
                ::std::result::Result::Ok(mut value) => {
                    return ::std::result::Result::Ok(value);
                },
                ::std::result::Result::Err(_) => {
                    return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("unexpected job rejection")));
                },
            }
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("worker unavailable")));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_776169745f6576656e74<'a>(mut control: &'a ::nagi_runtime::actor::Control, mut name: &'a ::std::primitive::str, mut generation: ::std::primitive::i64, mut kind: ::nagi_runtime::actor::EventKind) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut control: &::nagi_runtime::actor::Control = control;
    let mut name: &::std::primitive::str = name;
    let mut reached: ::std::primitive::bool = false;
    while !(reached) {
        match (::nagi_runtime::actor::next_event_timeout(control, 5000i64)).await {
            ::std::result::Result::Ok(mut next) => {
                match next {
                    ::std::option::Option::Some(mut event) => {
                        if ((((event).child_name() == name) && ((event).kind() == kind)) && ((event).generation() == generation)) {
                            reached = true;
                        }
                    },
                    ::std::option::Option::None => {
                        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("supervisor stopped before the expected event")));
                    },
                }
            },
            ::std::result::Result::Err(mut problem) => {
                if ((problem).kind() == ::nagi_runtime::actor::WaitKind::TIMEOUT) {
                    return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("supervisor event deadline exceeded")));
                }
                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("invalid supervisor event deadline")));
            },
        }
    }
    return ::std::result::Result::Ok(assert!(reached));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6578657263697365<'a>(mut packing: &'a ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>, mut audit: &'a ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>, mut control: &'a ::nagi_runtime::actor::Control, mut connector_events: &'a ::nagi_runtime::actor::Control) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73, ::nagi_runtime::Error> {
    let mut packing: &::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72> = packing;
    let mut audit: &::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72> = audit;
    let mut control: &::nagi_runtime::actor::Control = control;
    let mut connector_events: &::nagi_runtime::actor::Control = connector_events;
    match (::nagi_runtime::actor::ready(packing, 5000i64)).await {
        ::std::result::Result::Ok(_) => {
            assert!(true);
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("packing worker did not start")));
        },
    }
    match (::nagi_runtime::actor::ready(audit, 5000i64)).await {
        ::std::result::Result::Ok(_) => {
            assert!(true);
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("audit worker did not start")));
        },
    }
    assert!((((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(packing, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Process { units: 5i64 })).await)? == 5i64));
    assert!((((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(audit, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Process { units: 7i64 })).await)? == 7i64));
    match (::nagi_runtime::actor::call(packing, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Process { units: -(1i64) }, 5000i64, 5000i64)).await {
        ::std::result::Result::Ok(mut reply) => {
            match reply {
                ::std::result::Result::Ok(_) => {
                    return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("invalid job was accepted")));
                },
                ::std::result::Result::Err(mut problem) => {
                    match problem {
                        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72::InvalidUnits => {
                            println!("{}", "job rejected: packing total stays 5");
                        },
                    }
                },
            }
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("business error was lost")));
        },
    }
    assert!((((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(packing, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Read)).await)? == 5i64));
    match (::nagi_runtime::actor::call(packing, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::FailWorker, 5000i64, 5000i64)).await {
        ::std::result::Result::Ok(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("worker failure returned a reply")));
        },
        ::std::result::Result::Err(mut problem) => {
            assert!(((problem).kind() == ::nagi_runtime::actor::CallKind::REPLY_LOST));
        },
    }
    assert!((((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(audit, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Process { units: 2i64 })).await)? == 9i64));
    let mut packing_name: ::std::string::String = ::std::string::String::from("packing");
    ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_776169745f6576656e74(control, (packing_name).as_str(), 2i64, ::nagi_runtime::actor::EventKind::STARTED)).await)?;
    assert!((((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(packing, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Read)).await)? == 0i64));
    println!("{}", "packing restarted: total reset to 0; audit continued at 9");
    let mut connector_name: ::std::string::String = ::std::string::String::from("connector");
    ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_776169745f6576656e74(connector_events, (connector_name).as_str(), 2i64, ::nagi_runtime::actor::EventKind::READY)).await)?;
    assert!(((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f737461727473() == 2i64) && (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f616374697665() == 1i64)));
    assert!((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f636c65616e757073() == 1i64));
    let mut packing_total: ::std::primitive::i64 = ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(packing, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Process { units: 4i64 })).await)?;
    let mut audit_total: ::std::primitive::i64 = ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374(audit, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Read)).await)?;
    assert!(((packing_total == 4i64) && (audit_total == 9i64)));
    println!("{}", "connector panic recovered: generation 2 is active");
    return ::std::result::Result::Ok(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 { packing: packing_total, audit: audit_total });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_66696e697368(mut packing: ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>, mut audit: ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72>, mut control: ::nagi_runtime::actor::Control, mut events: ::nagi_runtime::actor::Control, mut connector_events: ::nagi_runtime::actor::Control) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut outcome: ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73, ::nagi_runtime::Error> = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6578657263697365(&(packing), &(audit), &(control), &(connector_events))).await;
    ((::nagi_runtime::actor::shutdown(&(control))).await)?;
    let mut totals: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 = (outcome)?;
    assert!((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f616374697665() == 0i64));
    assert!((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f636c65616e757073() == 2i64));
    match (::nagi_runtime::actor::call(&(packing), __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62::Read, 0i64, 5000i64)).await {
        ::std::result::Result::Ok(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("stopped worker accepted a job")));
        },
        ::std::result::Result::Err(mut problem) => {
            assert!(((problem).kind() == ::nagi_runtime::actor::CallKind::STOPPED));
        },
    }
    let mut packing_starts: ::std::primitive::i64 = 0i64;
    let mut audit_starts: ::std::primitive::i64 = 0i64;
    let mut connector_started: ::std::primitive::i64 = 0i64;
    let mut connector_ready: ::std::primitive::i64 = 0i64;
    let mut failed: ::std::primitive::i64 = 0i64;
    let mut panicked: ::std::primitive::i64 = 0i64;
    let mut restarts: ::std::primitive::i64 = 0i64;
    let mut stopped: ::std::primitive::i64 = 0i64;
    let mut shutdown: ::std::primitive::i64 = 0i64;
    let mut reading: ::std::primitive::bool = true;
    while reading {
        match ((::nagi_runtime::actor::next_event(&(events))).await)? {
            ::std::option::Option::Some(mut event) => {
                if ((event).kind() == ::nagi_runtime::actor::EventKind::STARTED) {
                    if ((event).child_name() == "packing") {
                        packing_starts = (packing_starts + 1i64);
                    }
                    if ((event).child_name() == "audit") {
                        audit_starts = (audit_starts + 1i64);
                    }
                    if ((event).child_name() == "connector") {
                        connector_started = (connector_started + 1i64);
                    }
                }
                if ((event).kind() == ::nagi_runtime::actor::EventKind::READY) {
                    assert!((((event).child_name() == "connector") && ((event).generation() == 2i64)));
                    connector_ready = (connector_ready + 1i64);
                }
                if ((event).kind() == ::nagi_runtime::actor::EventKind::FAILED) {
                    failed = (failed + 1i64);
                }
                if ((event).kind() == ::nagi_runtime::actor::EventKind::PANICKED) {
                    panicked = (panicked + 1i64);
                }
                if ((event).kind() == ::nagi_runtime::actor::EventKind::RESTART_SCHEDULED) {
                    restarts = (restarts + 1i64);
                }
                if ((event).kind() == ::nagi_runtime::actor::EventKind::STOPPED) {
                    stopped = (stopped + 1i64);
                }
                if ((event).kind() == ::nagi_runtime::actor::EventKind::SHUTDOWN) {
                    shutdown = (shutdown + 1i64);
                }
                assert!(((event).kind() != ::nagi_runtime::actor::EventKind::LAGGED));
                assert!(((event).kind() != ::nagi_runtime::actor::EventKind::INTENSITY_EXCEEDED));
            },
            ::std::option::Option::None => {
                reading = false;
            },
        }
    }
    assert!(((packing_starts == 2i64) && (audit_starts == 1i64)));
    assert!((((connector_started == 2i64) && (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f737461727473() == 2i64)) && (connector_ready == 1i64)));
    assert!((((failed == 1i64) && (panicked == 1i64)) && (restarts == 2i64)));
    assert!(((stopped == 3i64) && (shutdown == 1i64)));
    let mut report: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 = __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 { packing_total: (totals).packing, audit_total: (totals).audit, packing_starts: packing_starts, audit_starts: audit_starts, connector_starts: connector_started, failed_events: failed, panicked_events: panicked, restarts: restarts, stopped_children: stopped, shutdown_events: shutdown, connector_active: crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f616374697665(), connector_cleanups: crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f636c65616e757073() };
    let mut encoded: ::std::string::String = (::nagi_runtime::encode(&report))?;
    return ::std::result::Result::Ok(println!("{}", encoded));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut group: ::nagi_runtime::actor::Supervisor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874> = ::nagi_runtime::actor::supervisor::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874>(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874 { initial: 0i64 }, ::nagi_runtime::actor::default_options());
    let mut packing: ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72> = (::nagi_runtime::actor::register(&(group), "packing", crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6372656174655f776f726b6572, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_70726f636573735f6a6f62, ::nagi_runtime::actor::default_actor_options()))?;
    let mut audit: ::nagi_runtime::actor::Actor<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62, ::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72> = (::nagi_runtime::actor::register(&(group), "audit", crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6372656174655f776f726b6572, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_70726f636573735f6a6f62, ::nagi_runtime::actor::default_actor_options()))?;
    (::nagi_runtime::actor::task_with_ready(&(group), "connector", crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72756e5f636f6e6e6563746f72, ::nagi_runtime::actor::RestartPolicy::TRANSIENT))?;
    let mut control: ::nagi_runtime::actor::Control = ::nagi_runtime::actor::control(&(group));
    let mut events: ::nagi_runtime::actor::Control = ::nagi_runtime::actor::clone_control(&(control));
    let mut connector_events: ::nagi_runtime::actor::Control = ::nagi_runtime::actor::clone_control(&(control));
    {
        let mut __scope = ::nagi_runtime::Scope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6d6f6e69746f72(group); __scope.spawn(async move { __nagi_spawn_future.await }); }
            { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_66696e697368(packing, audit, control, events, connector_events); __scope.spawn(async move { __nagi_spawn_future.await }); }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { __scope.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__scope.join().await)?;
    }
    return ::std::result::Result::Ok(assert!(true));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_436f6e74657874 as Context;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_576f726b65725374617465 as WorkerState;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_546f74616c73 as Totals;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_c_5265706f7274 as Report;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f62 as Job;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_e_4a6f624572726f72 as JobError;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f72 as connector;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f737461727473 as connector_starts;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f616374697665 as connector_active;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_636f6e6e6563746f725f636c65616e757073 as connector_cleanups;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6372656174655f776f726b6572 as create_worker;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_70726f636573735f6a6f62 as process_job;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72756e5f636f6e6e6563746f72 as run_connector;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6d6f6e69746f72 as monitor;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_72657175657374 as request;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_776169745f6576656e74 as wait_event;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_6578657263697365 as exercise;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f737570657276697365642d776f726b65722f6d61696e2e6e616769_f_66696e697368 as finish;
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
fn main() { ::nagi_runtime::block_on(async {
if let ::std::result::Result::Err(e) = __nagi_main().await { eprintln!("{}",e); ::std::process::exit(1); }
}); }

#[path = "/workspace/Nagi-security-sf05/test-nagi-code/application-examples/supervised-worker/native.rs"]
mod native;
