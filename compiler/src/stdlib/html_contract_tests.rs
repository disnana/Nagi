//! Handwritten SF04 metadata oracle.
//!
//! Expected names and properties below come from the frozen SF04 contract,
//! not from the registry under test. Keep lookup string-based so this test
//! can be added before the canonical Html registry enums are introduced.
use super::{Passing, TypeArgumentRole};
use crate::ast::{DefKind, ModuleId};

struct ResourceExpected {
    name: &'static str,
    capabilities: [bool; 5], // Copy, equality, storage, shared, Debug
    rust_path: &'static str,
}

const RESOURCES: &[ResourceExpected] = &[
    ResourceExpected {
        name: "HtmlPolicy",
        capabilities: [false, false, true, true, false],
        rust_path: "::nagi_runtime::html::HtmlPolicy",
    },
    ResourceExpected {
        name: "HtmlAttributes",
        capabilities: [false, false, true, true, false],
        rust_path: "::nagi_runtime::html::HtmlAttributes",
    },
    ResourceExpected {
        name: "NavigationUrl",
        capabilities: [false, false, true, true, false],
        rust_path: "::nagi_runtime::html::NavigationUrl",
    },
    ResourceExpected {
        name: "HtmlFragment",
        capabilities: [false, false, true, true, false],
        rust_path: "::nagi_runtime::html::HtmlFragment",
    },
    ResourceExpected {
        name: "HtmlDocument",
        capabilities: [false, false, true, true, false],
        rust_path: "::nagi_runtime::html::HtmlDocument",
    },
    ResourceExpected {
        name: "HtmlTag",
        capabilities: [true, true, true, true, true],
        rust_path: "::nagi_runtime::html::HtmlTag",
    },
];

struct OperationExpected {
    name: &'static str,
    signature: &'static str,
    parameters: &'static [Passing],
    rust_path: &'static str,
}

const HTML_OPERATIONS: &[OperationExpected] = &[
    OperationExpected {
        name: "policy",
        signature: "(base_origin: view[str]) -> Result[HtmlPolicy, Error]",
        parameters: &[Passing::Reference],
        rust_path: "::nagi_runtime::html::policy",
    },
    OperationExpected {
        name: "limits",
        signature: "(policy: HtmlPolicy, output_bytes: i64, input_bytes: i64, nodes: i64, depth: i64, urls: i64, url_bytes: i64) -> Result[HtmlPolicy, Error]",
        parameters: &[
            Passing::Move,
            Passing::Move,
            Passing::Move,
            Passing::Move,
            Passing::Move,
            Passing::Move,
            Passing::Move,
        ],
        rust_path: "::nagi_runtime::html::limits",
    },
    OperationExpected {
        name: "empty",
        signature: "(policy: view[HtmlPolicy]) -> HtmlFragment",
        parameters: &[Passing::Reference],
        rust_path: "::nagi_runtime::html::empty",
    },
    OperationExpected {
        name: "text",
        signature: "(policy: view[HtmlPolicy], value: view[str]) -> Result[HtmlFragment, Error]",
        parameters: &[Passing::Reference, Passing::Reference],
        rust_path: "::nagi_runtime::html::text",
    },
    OperationExpected {
        name: "attributes",
        signature: "(policy: view[HtmlPolicy]) -> HtmlAttributes",
        parameters: &[Passing::Reference],
        rust_path: "::nagi_runtime::html::attributes",
    },
    OperationExpected {
        name: "title",
        signature: "(attributes: HtmlAttributes, value: view[str]) -> Result[HtmlAttributes, Error]",
        parameters: &[Passing::Move, Passing::Reference],
        rust_path: "::nagi_runtime::html::title",
    },
    OperationExpected {
        name: "href",
        signature: "(attributes: HtmlAttributes, value: NavigationUrl) -> Result[HtmlAttributes, Error]",
        parameters: &[Passing::Move, Passing::Move],
        rust_path: "::nagi_runtime::html::href",
    },
    OperationExpected {
        name: "navigation",
        signature: "(policy: view[HtmlPolicy], value: view[str]) -> Result[NavigationUrl, Error]",
        parameters: &[Passing::Reference, Passing::Reference],
        rust_path: "::nagi_runtime::html::navigation",
    },
    OperationExpected {
        name: "element",
        signature: "(policy: view[HtmlPolicy], tag: HtmlTag, attributes: HtmlAttributes, children: HtmlFragment) -> Result[HtmlFragment, Error]",
        parameters: &[
            Passing::Reference,
            Passing::Move,
            Passing::Move,
            Passing::Move,
        ],
        rust_path: "::nagi_runtime::html::element",
    },
    OperationExpected {
        name: "join",
        signature: "(policy: view[HtmlPolicy], left: HtmlFragment, right: HtmlFragment) -> Result[HtmlFragment, Error]",
        parameters: &[Passing::Reference, Passing::Move, Passing::Move],
        rust_path: "::nagi_runtime::html::join",
    },
    OperationExpected {
        name: "copy_fragment",
        signature: "(policy: view[HtmlPolicy], fragment: view[HtmlFragment]) -> Result[HtmlFragment, Error]",
        parameters: &[Passing::Reference, Passing::Reference],
        rust_path: "::nagi_runtime::html::copy_fragment",
    },
    OperationExpected {
        name: "document",
        signature: "(policy: view[HtmlPolicy], title: view[str], body: HtmlFragment) -> Result[HtmlDocument, Error]",
        parameters: &[Passing::Reference, Passing::Reference, Passing::Move],
        rust_path: "::nagi_runtime::html::document",
    },
];

const HTML_TAGS: &[&str] = &[
    "DIV", "SPAN", "P", "H1", "H2", "STRONG", "EM", "UL", "OL", "LI", "A", "BR",
];

fn only_named_function(module: &ModuleId, name: &str) -> super::Operation {
    let matches: Vec<_> = super::definitions(module)
        .into_iter()
        .filter(|definition| definition.id.kind == DefKind::Function && definition.id.name == name)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one registered function named {name}"
    );
    super::operation(&matches[0].symbol)
        .unwrap_or_else(|| panic!("function {name} has no registered operation metadata"))
}

fn assert_operation(
    operation: super::Operation,
    expected_name: &str,
    expected_module: &str,
    expected_module_id: &str,
    expected_signature: &str,
    expected_parameters: &[Passing],
    expected_rust_path: &str,
) {
    let info = super::operation_info(operation);
    assert_eq!(info.name, expected_name);
    assert_eq!(super::module_info(info.module).name, expected_module);
    assert_eq!(super::operation_module(operation).0, expected_module_id);
    assert_eq!(info.rust_path, expected_rust_path);
    assert_eq!(info.arity, expected_parameters.len());
    assert_eq!(info.parameters, expected_parameters);
    assert_eq!(info.generic_arity, 0);
    assert!(info.type_parameters.is_empty());
    assert!(!info.asynchronous);
    assert!(!info.emit_type_arguments);
    assert_eq!(info.borrow_owner, None);
    assert_eq!(info.signature, expected_signature);
}

#[test]
fn canonical_html_metadata_matches_the_frozen_contract() {
    // This is intentionally the first assertion: before implementation lands,
    // the expected RED is a missing canonical metadata module, not a parser or
    // unresolved-import failure from a proposed API.
    let html_module = super::module("std.html");
    assert_eq!(
        html_module.as_ref().map(|module| module.0.as_str()),
        Some("stdlib:std.html"),
        "SF04 metadata contract: canonical std.html module is not registered"
    );
    let html_module = html_module.expect("the canonical module assertion above must pass");

    let html_module_info = super::MODULES
        .iter()
        .copied()
        .map(super::module_info)
        .find(|info| info.name == "std.html")
        .expect("registered std.html module must have module metadata");
    assert_eq!(html_module_info.id, "stdlib:std.html");
    assert_eq!(html_module_info.rust_namespace, "::nagi_runtime::html");

    let definitions = super::definitions(&html_module);
    let actual_resources: Vec<_> = definitions
        .iter()
        .filter(|definition| definition.id.kind == DefKind::Resource)
        .map(|definition| definition.id.name.as_str())
        .collect();
    assert_eq!(
        actual_resources,
        RESOURCES
            .iter()
            .map(|expected| expected.name)
            .collect::<Vec<_>>(),
        "std.html resources must match the independent complete inventory in contract order"
    );
    assert_eq!(RESOURCES.len(), 6);

    for expected in RESOURCES {
        let resource = super::resource_named(&html_module, expected.name)
            .unwrap_or_else(|| panic!("missing std.html resource {}", expected.name));
        let info = super::resource_info(resource);
        assert_eq!(info.name, expected.name);
        assert_eq!(super::module_info(info.module).name, "std.html");
        assert_eq!(super::resource_module(resource).0, "stdlib:std.html");
        assert_eq!(info.rust_path, expected.rust_path);
        assert_eq!(info.arity, 0);
        assert!(info.type_parameters.is_empty());
        assert!(info.inline_type_arguments.is_empty());
        assert_eq!(
            [
                info.copy,
                info.equality,
                info.storage,
                info.shared,
                info.debug
            ],
            expected.capabilities,
            "resource capability mismatch for {}",
            expected.name
        );
        assert!(
            !super::native_serde_supported(resource),
            "{} must not derive native Serde",
            expected.name
        );
        assert!(
            matches!(
                super::resource_contract(resource).lifecycle,
                super::ResourceLifecycle::Unspecified
            ),
            "{} must retain the Unspecified lifecycle",
            expected.name
        );
        assert!(!super::requires_same_task(resource));
        for role in [
            TypeArgumentRole::InlinePayload,
            TypeArgumentRole::SharedPayload,
            TypeArgumentRole::IndirectProtocol,
            TypeArgumentRole::CallbackSignature,
            TypeArgumentRole::NominalPhantom,
        ] {
            assert!(
                super::type_argument_positions(resource, role).is_empty(),
                "{} unexpectedly declares generic role {role:?}",
                expected.name
            );
        }
    }

    let html_operations: Vec<_> = definitions
        .iter()
        .filter(|definition| definition.id.kind == DefKind::Function)
        .map(|definition| definition.id.name.as_str())
        .collect();
    assert_eq!(
        html_operations,
        HTML_OPERATIONS
            .iter()
            .map(|expected| expected.name)
            .collect::<Vec<_>>(),
        "std.html operations must match the independent complete inventory in contract order"
    );
    // Twelve operations live in std.html; the thirteenth SF04 operation is
    // the checked response constructor in the existing std.http.server module.
    assert_eq!(HTML_OPERATIONS.len() + 1, 13);

    for expected in HTML_OPERATIONS {
        let operation = only_named_function(&html_module, expected.name);
        assert_operation(
            operation,
            expected.name,
            "std.html",
            "stdlib:std.html",
            expected.signature,
            expected.parameters,
            expected.rust_path,
        );
    }

    let http_module = super::module("std.http.server")
        .expect("existing std.http.server module must be registered");
    let html_response = only_named_function(&http_module, "html_response");

    // The public SF04 contract writes this cross-module type as
    // `html.HtmlDocument`. Standard operation metadata stores resource names
    // bare, including existing HttpServer signatures that reference AuthScope
    // and Grant from the Auth module (stdlib/security.rs). Check the canonical
    // resource identity directly rather than confusing that registry spelling
    // with a source import alias.
    let html_document = super::resource_named(&html_module, "HtmlDocument")
        .expect("the response argument must resolve to the canonical HtmlDocument resource");
    let html_document_id = super::resource_id(html_document);
    assert_eq!(html_document_id.module.0, "stdlib:std.html");
    assert_eq!(html_document_id.kind, DefKind::Resource);
    assert_eq!(html_document_id.name, "HtmlDocument");

    assert_operation(
        html_response,
        "html_response",
        "std.http.server",
        "stdlib:std.http.server",
        "(status: Status, document: HtmlDocument) -> Response",
        &[Passing::Move, Passing::Move],
        "::nagi_runtime::http_server::html_response",
    );

    let html_tag = super::resource_named(&html_module, "HtmlTag")
        .expect("HtmlTag is part of the complete resource inventory");
    let actual_tags: Vec<_> = super::constants(html_tag)
        .iter()
        .map(|constant| (constant.name, constant.native_name))
        .collect();
    assert_eq!(
        actual_tags,
        HTML_TAGS
            .iter()
            .map(|name| (*name, *name))
            .collect::<Vec<_>>(),
        "HtmlTag constants and native names must match the closed ordered contract"
    );
    assert_eq!(HTML_TAGS.len(), 12);
}
