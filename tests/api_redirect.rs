//! `ApiClient` must not leak `X-API-Key` through a cross-host redirect
//! (reqwest strips only Authorization / Cookie / Proxy-Authorization /
//! WWW-Authenticate). Same-host redirects keep working.

use nadfun_sdk::{ApiClient, Network};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> ApiClient {
    ApiClient::new(Network::Mainnet)
        .with_api_key("nadfun_secret_key".to_string())
        .with_api_url(server.uri())
}

#[tokio::test]
async fn cross_host_redirect_is_refused() {
    let api_host = MockServer::start().await;
    let other_host = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token/x"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/steal", other_host.uri())),
        )
        .mount(&api_host)
        .await;
    // Any request reaching the other host (with or without the key) fails the test.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&other_host)
        .await;

    let result = client(&api_host).get("/token/x").send().await;
    assert!(
        result.is_err(),
        "cross-host redirect was followed: {result:?}"
    );
    other_host.verify().await;
}

#[tokio::test]
async fn same_host_redirect_is_followed() {
    let api_host = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/old"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", format!("{}/new", api_host.uri())),
        )
        .mount(&api_host)
        .await;
    Mock::given(method("GET"))
        .and(path("/new"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&api_host)
        .await;

    let response = client(&api_host).get("/old").send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
}
