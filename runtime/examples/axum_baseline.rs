use nagi_runtime as rt;
use rt::axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
struct User {
    id: i64,
    name: String,
    age: i32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUser {
    name: String,
    age: i32,
}
fn main() {
    rt::block_on(async {
        let router = Router::new()
            .route(
                "/small",
                get(|| async {
                    rt::response(Ok(User {
                        id: 1,
                        name: "tp-li".into(),
                        age: 18,
                    }))
                }),
            )
            .route(
                "/echo",
                post(|body: Bytes| async move { rt::response(rt::decode::<CreateUser>(&body)) }),
            );
        rt::serve(router, 8082).await.unwrap();
    });
}
