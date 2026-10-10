use nagi_runtime::html::{
    self, HtmlAttributes, HtmlDocument, HtmlFragment, HtmlPolicy, HtmlTag, NavigationUrl,
};
use nagi_runtime::http_server::{self, Status};

fn immutable_owned<T: Send + Sync + 'static>() {}

#[test]
fn opaque_api_owns_data_and_is_transferable() {
    immutable_owned::<HtmlPolicy>();
    immutable_owned::<HtmlAttributes>();
    immutable_owned::<NavigationUrl>();
    immutable_owned::<HtmlFragment>();
    immutable_owned::<HtmlDocument>();
    immutable_owned::<HtmlTag>();
    let policy = html::policy("https://example.invalid").unwrap();
    let link = html::navigation(&policy, "/items/7?view=details").unwrap();
    let attrs = html::href(html::attributes(&policy), link).unwrap();
    let child = html::text(&policy, "通常の item").unwrap();
    let fragment = html::element(&policy, HtmlTag::A, attrs, child).unwrap();
    let copied = html::copy_fragment(&policy, &fragment).unwrap();
    let body = html::join(&policy, fragment, copied).unwrap();
    let document = html::document(&policy, "Item list", body).unwrap();
    let response = http_server::html_response(Status::CREATED, document);
    assert_eq!(response.status(), Status::CREATED);
    assert!(response.body().starts_with(b"<!doctype html>"));
    assert!(http_server::append_header_text(response, "Content-Encoding", "gzip").is_err());
}
