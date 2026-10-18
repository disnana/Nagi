//! Process-local witness that this crate instance installed its reviewed JWT provider first.
//!
//! Call only after the caller has completed finite configuration validation. Every failed
//! public installation is deliberately treated as a conflict; the returned provider reference
//! is never inspected.

use std::sync::OnceLock;

use jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER;

pub(crate) struct ProviderProof {
    _sealed: (),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderConflict {
    AlreadyInitialized,
}

static NAGI_PROVIDER: OnceLock<Result<ProviderProof, ProviderConflict>> = OnceLock::new();

pub(crate) fn require_provider_proof() -> Result<&'static ProviderProof, ProviderConflict> {
    match NAGI_PROVIDER.get_or_init(|| {
        DEFAULT_PROVIDER
            .install_default()
            .map(|()| ProviderProof { _sealed: () })
            .map_err(|_| ProviderConflict::AlreadyInitialized)
    }) {
        Ok(proof) => Ok(proof),
        Err(conflict) => Err(*conflict),
    }
}
