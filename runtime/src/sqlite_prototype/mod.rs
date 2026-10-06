// ADR 010のprivate一接続prototype。公開Pool/APIでもpool algorithmでもない。
mod session;
use session::*;
#[cfg(test)]
mod acquire_tests;
mod adapter;
#[cfg(test)]
mod adapter_tests;
mod comparison;
#[cfg(test)]
mod tests;
