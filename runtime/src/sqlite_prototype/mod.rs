// ADR 010のprivate一接続prototype。公開Pool/APIでもpool algorithmでもない。
mod session;
use session::*;
#[cfg(test)]
mod tests;
