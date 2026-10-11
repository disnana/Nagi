//! Frozen SF04 opaque immutable HTML values; never an authorization proof.
use super::*;

const fn plain(name: &'static str, path: &'static str, copy: bool) -> ResourceContract {
    ResourceContract::new(
        ResourceInfo {
            module: StandardModule::Html,
            name,
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: path,
            copy,
            equality: copy,
            storage: true,
            shared: true,
            debug: copy,
        },
        &[],
        &[],
        &[],
        &[],
    )
}
static POLICY: ResourceContract = plain("HtmlPolicy", "::nagi_runtime::html::HtmlPolicy", false);
static ATTRIBUTES: ResourceContract = plain(
    "HtmlAttributes",
    "::nagi_runtime::html::HtmlAttributes",
    false,
);
static NAVIGATION: ResourceContract = plain(
    "NavigationUrl",
    "::nagi_runtime::html::NavigationUrl",
    false,
);
static FRAGMENT: ResourceContract =
    plain("HtmlFragment", "::nagi_runtime::html::HtmlFragment", false);
static DOCUMENT: ResourceContract =
    plain("HtmlDocument", "::nagi_runtime::html::HtmlDocument", false);
static TAG: ResourceContract = plain("HtmlTag", "::nagi_runtime::html::HtmlTag", true);

pub(super) fn resource_contract(resource: Resource) -> &'static ResourceContract {
    match resource {
        Resource::HtmlPolicy => &POLICY,
        Resource::HtmlAttributes => &ATTRIBUTES,
        Resource::NavigationUrl => &NAVIGATION,
        Resource::HtmlFragment => &FRAGMENT,
        Resource::HtmlDocument => &DOCUMENT,
        Resource::HtmlTag => &TAG,
        _ => unreachable!("non-HTML resource dispatched to HTML contract"),
    }
}
macro_rules! operation {
    ($module:ident, $name:literal, $path:literal, $passing:expr, $signature:literal) => {{
        static INFO: OperationInfo = OperationInfo {
            module: StandardModule::$module,
            name: $name,
            rust_path: $path,
            arity: ($passing as &[Passing]).len(),
            generic_arity: 0,
            type_parameters: &[],
            asynchronous: false,
            emit_type_arguments: false,
            parameters: $passing,
            borrow_owner: None,
            signature: $signature,
        };
        &INFO
    }};
}
pub(super) fn operation_info(operation: Operation) -> &'static OperationInfo {
    use Passing::{Move as M, Reference as R};
    match operation {
        Operation::HtmlPolicy => operation!(Html, "policy", "::nagi_runtime::html::policy", &[R], "(base_origin: view[str]) -> Result[HtmlPolicy, Error]"),
        Operation::HtmlLimits => operation!(Html, "limits", "::nagi_runtime::html::limits", &[M,M,M,M,M,M,M], "(policy: HtmlPolicy, output_bytes: i64, input_bytes: i64, nodes: i64, depth: i64, urls: i64, url_bytes: i64) -> Result[HtmlPolicy, Error]"),
        Operation::HtmlEmpty => operation!(Html, "empty", "::nagi_runtime::html::empty", &[R], "(policy: view[HtmlPolicy]) -> HtmlFragment"),
        Operation::HtmlText => operation!(Html, "text", "::nagi_runtime::html::text", &[R,R], "(policy: view[HtmlPolicy], value: view[str]) -> Result[HtmlFragment, Error]"),
        Operation::HtmlAttributes => operation!(Html, "attributes", "::nagi_runtime::html::attributes", &[R], "(policy: view[HtmlPolicy]) -> HtmlAttributes"),
        Operation::HtmlTitle => operation!(Html, "title", "::nagi_runtime::html::title", &[M,R], "(attributes: HtmlAttributes, value: view[str]) -> Result[HtmlAttributes, Error]"),
        Operation::HtmlHref => operation!(Html, "href", "::nagi_runtime::html::href", &[M,M], "(attributes: HtmlAttributes, value: NavigationUrl) -> Result[HtmlAttributes, Error]"),
        Operation::HtmlNavigation => operation!(Html, "navigation", "::nagi_runtime::html::navigation", &[R,R], "(policy: view[HtmlPolicy], value: view[str]) -> Result[NavigationUrl, Error]"),
        Operation::HtmlElement => operation!(Html, "element", "::nagi_runtime::html::element", &[R,M,M,M], "(policy: view[HtmlPolicy], tag: HtmlTag, attributes: HtmlAttributes, children: HtmlFragment) -> Result[HtmlFragment, Error]"),
        Operation::HtmlJoin => operation!(Html, "join", "::nagi_runtime::html::join", &[R,M,M], "(policy: view[HtmlPolicy], left: HtmlFragment, right: HtmlFragment) -> Result[HtmlFragment, Error]"),
        Operation::HtmlCopyFragment => operation!(Html, "copy_fragment", "::nagi_runtime::html::copy_fragment", &[R,R], "(policy: view[HtmlPolicy], fragment: view[HtmlFragment]) -> Result[HtmlFragment, Error]"),
        Operation::HtmlDocument => operation!(Html, "document", "::nagi_runtime::html::document", &[R,R,M], "(policy: view[HtmlPolicy], title: view[str], body: HtmlFragment) -> Result[HtmlDocument, Error]"),
        Operation::HttpHtmlResponse => operation!(HttpServer, "html_response", "::nagi_runtime::http_server::html_response", &[M,M], "(status: Status, document: HtmlDocument) -> Response"),
        _ => unreachable!("non-HTML operation dispatched to HTML contract"),
    }
}
pub(super) const TAG_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "DIV",
        native_name: "DIV",
    },
    ConstantInfo {
        name: "SPAN",
        native_name: "SPAN",
    },
    ConstantInfo {
        name: "P",
        native_name: "P",
    },
    ConstantInfo {
        name: "H1",
        native_name: "H1",
    },
    ConstantInfo {
        name: "H2",
        native_name: "H2",
    },
    ConstantInfo {
        name: "STRONG",
        native_name: "STRONG",
    },
    ConstantInfo {
        name: "EM",
        native_name: "EM",
    },
    ConstantInfo {
        name: "UL",
        native_name: "UL",
    },
    ConstantInfo {
        name: "OL",
        native_name: "OL",
    },
    ConstantInfo {
        name: "LI",
        native_name: "LI",
    },
    ConstantInfo {
        name: "A",
        native_name: "A",
    },
    ConstantInfo {
        name: "BR",
        native_name: "BR",
    },
];
