'use strict';
const fixtures = [
  ['nagi', 'import std.http.server as http\nfrom std.http.server import Request as Incoming, Status as Code\n' +
    'def label(prefix: view[str], status: Code) -> i64:\n    print(prefix)\n    return status.value\n' +
    'def handle(request: Incoming) -> Result[http.Response, Error]:\n' +
    '    json_content = try http.is_json_content_type(view(request))\n' +
    '    print(json_content)\n' +
    '    optional = try http.header_text(view(request), view("authorization"))\n' +
    '    match optional:\n        case Some(token):\n            print(token)\n        case None:\n            print("missing")\n' +
    '    if request.is_get:\n        print(request.path)\n' +
    '    return ok(http.text(Code.OK, view("hello")))\n' +
    'def main():\n    print(label(view("😀"), Code.UNAUTHORIZED))\n'],
  ['low', 'import std.http.server as http;\nfrom std.http.server import Request as Incoming, Status as Code;\n' +
    'fn label(prefix: view[str], status: Code) -> i64 { print(prefix); return status.value; }\n' +
    'fn handle(request: Incoming) -> Result[http.Response, Error] {\n' +
    '    let json_content = try http.is_json_content_type(view(request));\n' +
    '    print(json_content);\n' +
    '    let optional = try http.header_text(view(request), view("authorization"));\n' +
    '    match optional { case Some(token) { print(token); } case None { print("missing"); } }\n' +
    '    if request.is_get { print(request.path); }\n' +
    '    return ok(http.text(Code.OK, view("hello")));\n}\n' +
    'fn main() { print(label(view("😀"), Code.UNAUTHORIZED)); }\n'],
];
module.exports = { fixtures };
