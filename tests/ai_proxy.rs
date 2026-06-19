use reader_next::model::ai_proxy::{
    ai_proxy_timeout, build_ai_proxy_url, format_ai_proxy_upstream_error,
    validate_ai_proxy_image_url,
};

#[test]
fn ai_proxy_url_allows_known_model_paths() {
    let url =
        build_ai_proxy_url("https://api.example.test/", "/v1/chat/completions", false).unwrap();
    assert_eq!(url.as_str(), "https://api.example.test/v1/chat/completions");

    let speech_url =
        build_ai_proxy_url("https://api.example.test/", "/v1/audio/speech", false).unwrap();
    assert_eq!(
        speech_url.as_str(),
        "https://api.example.test/v1/audio/speech"
    );

    let responses_url =
        build_ai_proxy_url("https://api.example.test/", "/v1/responses", false).unwrap();
    assert_eq!(
        responses_url.as_str(),
        "https://api.example.test/v1/responses"
    );

    let response_typo_url =
        build_ai_proxy_url("https://api.example.test/", "/v1/response", false).unwrap();
    assert_eq!(
        response_typo_url.as_str(),
        "https://api.example.test/v1/responses"
    );

    let claude_url =
        build_ai_proxy_url("https://api.anthropic.com/", "/v1/messages", false).unwrap();
    assert_eq!(claude_url.as_str(), "https://api.anthropic.com/v1/messages");

    let gemini_url = build_ai_proxy_url(
        "https://generativelanguage.googleapis.com/",
        "/v1beta/models/gemini-2.5-pro:generateContent",
        false,
    )
    .unwrap();
    assert_eq!(
        gemini_url.as_str(),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:generateContent"
    );

    let gemini_openai_url = build_ai_proxy_url(
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "/v1/chat/completions",
        false,
    )
    .unwrap();
    assert_eq!(
        gemini_openai_url.as_str(),
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
    );

    let err = build_ai_proxy_url("https://api.example.test/", "/v1/models", false).unwrap_err();
    assert!(err.contains("unsupported proxy path"));
}

#[test]
fn ai_proxy_url_can_use_full_model_endpoint_without_appending_path() {
    let url = build_ai_proxy_url(
        "https://gateway.example.test/custom/chat?deployment=reader",
        "/v1/chat/completions",
        true,
    )
    .unwrap();

    assert_eq!(
        url.as_str(),
        "https://gateway.example.test/custom/chat?deployment=reader"
    );
}

#[test]
fn ai_proxy_url_rejects_non_http_targets() {
    let err = build_ai_proxy_url("file:///tmp/secret", "/v1/chat/completions", false).unwrap_err();
    assert!(err.contains("http"));

    let image_err = validate_ai_proxy_image_url("data:image/png;base64,abc").unwrap_err();
    assert!(image_err.contains("http"));
}

#[test]
fn ai_proxy_uses_model_sized_timeout() {
    assert!(ai_proxy_timeout().as_secs() >= 60);
}

#[test]
fn ai_proxy_formats_upstream_method_errors() {
    let message = format_ai_proxy_upstream_error(
        405,
        "<html><body><h1>Method Not Allowed</h1></body></html>",
    );

    assert!(message.contains("模型服务返回 405"));
    assert!(message.contains("Method Not Allowed"));
}
