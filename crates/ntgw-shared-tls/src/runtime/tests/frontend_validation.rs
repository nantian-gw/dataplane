#[tokio::test]
async fn terminate_tls_requires_client_certificate_when_frontend_validation_is_strict() -> Result<()>
{
    let (server_result, client_result) =
        run_frontend_validation_handshake("RequireClientCertificate", CLIENT_CERT_PEM, false)
            .await?;

    assert!(
        server_result.is_err() || client_result.is_err(),
        "strict frontend validation must reject a client without certificate"
    );

    let (server_result, client_result) =
        run_frontend_validation_handshake("RequireClientCertificate", CLIENT_CERT_PEM, true)
            .await?;

    assert!(
        server_result.is_ok(),
        "strict frontend validation should accept matching client cert: {server_result:?}"
    );
    assert!(
        client_result.is_ok(),
        "strict frontend validation client handshake failed: {client_result:?}"
    );
    Ok(())
}

#[tokio::test]
async fn terminate_tls_rejects_non_matching_client_certificate_when_frontend_validation_is_strict(
) -> Result<()> {
    let (server_result, client_result) =
        run_frontend_validation_handshake("RequireClientCertificate", SERVER_CERT_PEM, true)
            .await?;

    assert!(
        server_result.is_err() || client_result.is_err(),
        "strict frontend validation must reject a client cert signed by another CA; server_result={server_result:?}, client_result={client_result:?}"
    );
    Ok(())
}

#[tokio::test]
async fn terminate_tls_allows_insecure_fallback_clients() -> Result<()> {
    let (server_result, client_result) =
        run_frontend_validation_handshake("AllowInsecureFallback", SERVER_CERT_PEM, false).await?;

    assert!(
        server_result.is_ok(),
        "fallback should accept client without certificate: {server_result:?}"
    );
    assert!(
        client_result.is_ok(),
        "fallback client without certificate failed: {client_result:?}"
    );

    let (server_result, client_result) =
        run_frontend_validation_handshake("AllowInsecureFallback", SERVER_CERT_PEM, true).await?;

    assert!(
        server_result.is_ok(),
        "fallback should accept client with non-matching certificate: {server_result:?}"
    );
    assert!(
        client_result.is_ok(),
        "fallback client with non-matching certificate failed: {client_result:?}"
    );
    Ok(())
}
