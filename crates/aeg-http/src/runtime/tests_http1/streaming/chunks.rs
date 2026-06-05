async fn read_next_h1_chunk(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
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
    let mut chunk = vec![0; size];
    stream.read_exact(&mut chunk).await?;
    let mut chunk_end = [0; 2];
    stream.read_exact(&mut chunk_end).await?;
    if chunk_end != *b"\r\n" {
        return Err(anyhow!("expected chunk terminator"));
    }
    Ok(chunk)
}

async fn write_h1_chunk(stream: &mut TcpStream, payload: &[u8]) -> anyhow::Result<()> {
    stream
        .write_all(format!("{:X}\r\n", payload.len()).as_bytes())
        .await?;
    stream.write_all(payload).await?;
    stream.write_all(b"\r\n").await?;
    Ok(())
}
