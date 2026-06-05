async fn read_http_headers(stream: &mut TcpStream) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    loop {
        let byte = timeout(Duration::from_secs(2), stream.read_u8()).await??;
        buf.push(byte);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8(buf)?)
}

async fn read_http_response(stream: &mut TcpStream) -> anyhow::Result<String> {
    let headers = read_http_headers(stream).await?;
    let body = read_http_body(stream, &headers).await?;
    let mut raw = headers.into_bytes();
    raw.extend_from_slice(&body);
    Ok(String::from_utf8(raw)?)
}

async fn read_h2_body(mut body: h2::RecvStream) -> anyhow::Result<Vec<u8>> {
    read_h2_body_stream(&mut body).await
}

async fn read_h2_body_stream(body: &mut h2::RecvStream) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    while let Some(chunk) = body.data().await {
        let chunk = chunk?;
        body.flow_control().release_capacity(chunk.len())?;
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

async fn drive_h2_server_io(connection: &mut h2server::Connection<TcpStream, bytes::Bytes>) {
    let _ = timeout(Duration::from_millis(50), connection.accept()).await;
}

async fn read_http_body(stream: &mut TcpStream, headers: &str) -> anyhow::Result<Vec<u8>> {
    if let Some(content_length) = header_value(headers, "content-length") {
        let len = content_length
            .parse::<usize>()
            .context("parse content-length")?;
        let mut body = vec![0; len];
        stream.read_exact(&mut body).await?;
        return Ok(body);
    }

    if header_value(headers, "transfer-encoding")
        .map(|value| {
            value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
        })
        .unwrap_or(false)
    {
        let (body, _) = read_chunked_body_and_trailers(stream).await?;
        return Ok(body);
    }

    Ok(Vec::new())
}

async fn read_chunked_body_and_trailers(
    stream: &mut TcpStream,
) -> anyhow::Result<(Vec<u8>, String)> {
    let mut buf = Vec::new();
    loop {
        let mut size_line = Vec::new();
        loop {
            let byte = timeout(Duration::from_secs(2), stream.read_u8()).await??;
            size_line.push(byte);
            if size_line.ends_with(b"\r\n") {
                break;
            }
        }
        let size_line = String::from_utf8(size_line)?;
        let size_hex = size_line
            .trim_end()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim();
        let size = usize::from_str_radix(size_hex, 16)
            .with_context(|| format!("parse chunk size from {size_hex:?}"))?;
        if size == 0 {
            let trailers = read_http_trailers(stream).await?;
            return Ok((buf, trailers));
        }
        let mut chunk = vec![0; size];
        stream.read_exact(&mut chunk).await?;
        buf.extend_from_slice(&chunk);
        let mut chunk_end = [0; 2];
        stream.read_exact(&mut chunk_end).await?;
        if chunk_end != *b"\r\n" {
            return Err(anyhow!("expected chunk terminator"));
        }
    }
}

async fn read_http_trailers(stream: &mut TcpStream) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    loop {
        let byte = timeout(Duration::from_secs(2), stream.read_u8()).await??;
        buf.push(byte);
        if buf == b"\r\n" || buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8(buf)?)
}

fn header_value(headers: &str, name: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

async fn read_all_with_timeout(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0; 1024];
    loop {
        match timeout(Duration::from_secs(2), stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(read)) => out.extend_from_slice(&buf[..read]),
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => break,
        }
    }
    Ok(out)
}
