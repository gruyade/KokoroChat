// LLM Client integration tests
// Unit tests are in client.rs module

use super::client::*;

#[test]
fn test_openai_compatible_client_default() {
    let client = OpenAICompatibleClient::default();
    // クライアントが正常に生成されることを確認
    let _ = client;
}
