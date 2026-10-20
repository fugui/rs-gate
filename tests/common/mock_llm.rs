use wiremock::MockServer;

#[allow(dead_code)]
pub struct MockLlmCluster {
    pub server_a: MockServer,
    pub server_b: MockServer,
}

#[allow(dead_code)]
impl MockLlmCluster {
    pub async fn start() -> Self {
        let server_a = MockServer::start().await;
        let server_b = MockServer::start().await;
        Self { server_a, server_b }
    }
}
