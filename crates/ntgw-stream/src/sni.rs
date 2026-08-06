#[cfg(test)]
mod tests;

#[must_use]
pub fn extract_server_name(payload: &[u8]) -> Option<String> {
    let record = parse_tls_record(payload)?;
    let handshake = parse_client_hello(record)?;
    parse_server_name_extension(handshake)
}

pub(crate) fn tls_record_len(payload: &[u8]) -> Option<usize> {
    if payload.len() < 5 || payload[0] != 22 {
        return None;
    }

    let length = u16::from_be_bytes([payload[3], payload[4]]) as usize;
    Some(5 + length)
}

fn parse_tls_record(payload: &[u8]) -> Option<&[u8]> {
    let length = tls_record_len(payload)?;
    if payload.len() < length {
        return None;
    }

    Some(&payload[5..length])
}

fn parse_client_hello(payload: &[u8]) -> Option<&[u8]> {
    if payload.len() < 4 || payload[0] != 1 {
        return None;
    }

    let length = ((payload[1] as usize) << 16) | ((payload[2] as usize) << 8) | payload[3] as usize;
    if payload.len() < 4 + length {
        return None;
    }

    Some(&payload[4..4 + length])
}

fn parse_server_name_extension(payload: &[u8]) -> Option<String> {
    let mut cursor = payload;
    if cursor.len() < 34 {
        return None;
    }

    cursor = &cursor[34..];
    cursor = skip_vector(cursor, 1)?;
    cursor = skip_vector(cursor, 2)?;
    cursor = skip_vector(cursor, 1)?;

    let extensions_len = read_u16(cursor)? as usize;
    cursor = cursor.get(2..2 + extensions_len)?;

    while cursor.len() >= 4 {
        let ext_type = read_u16(cursor)?;
        let ext_len = read_u16(&cursor[2..])? as usize;
        let ext_payload = cursor.get(4..4 + ext_len)?;
        if ext_type == 0 {
            return parse_server_name_list(ext_payload);
        }
        cursor = cursor.get(4 + ext_len..)?;
    }

    None
}

fn parse_server_name_list(payload: &[u8]) -> Option<String> {
    let list_len = read_u16(payload)? as usize;
    let mut cursor = payload.get(2..2 + list_len)?;

    while cursor.len() >= 3 {
        let name_type = cursor[0];
        let name_len = read_u16(&cursor[1..])? as usize;
        let name = cursor.get(3..3 + name_len)?;
        if name_type == 0 {
            return std::str::from_utf8(name).ok().map(ToOwned::to_owned);
        }
        cursor = cursor.get(3 + name_len..)?;
    }

    None
}

fn skip_vector(payload: &[u8], len_bytes: usize) -> Option<&[u8]> {
    let len = match len_bytes {
        1 => payload.first().copied()? as usize,
        2 => read_u16(payload)? as usize,
        _ => return None,
    };

    payload.get(len_bytes + len..)
}

fn read_u16(payload: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes([*payload.first()?, *payload.get(1)?]))
}
