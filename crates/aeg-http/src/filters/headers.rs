use pingora::http::{RequestHeader, ResponseHeader};

use aeg_ir::HeaderModifier;

pub(super) fn apply_header_modifier<T: HeaderEditor>(
    headers: &mut T,
    modifier: &HeaderModifier,
) -> pingora::Result<()> {
    for name in &modifier.remove {
        headers.remove_header_value(name);
    }
    for header in &modifier.set {
        headers.insert_header_value(&header.name, &header.value)?;
    }
    for header in &modifier.add {
        headers.append_header_value(&header.name, &header.value)?;
    }

    Ok(())
}

pub(super) trait HeaderEditor {
    fn insert_header_value(&mut self, name: &str, value: &str) -> pingora::Result<()>;
    fn append_header_value(&mut self, name: &str, value: &str) -> pingora::Result<()>;
    fn remove_header_value(&mut self, name: &str);
}

impl HeaderEditor for RequestHeader {
    fn insert_header_value(&mut self, name: &str, value: &str) -> pingora::Result<()> {
        self.insert_header(name.to_string(), value.to_string())
    }

    fn append_header_value(&mut self, name: &str, value: &str) -> pingora::Result<()> {
        self.append_header(name.to_string(), value.to_string())
            .map(|_| ())
    }

    fn remove_header_value(&mut self, name: &str) {
        self.remove_header(name);
    }
}

impl HeaderEditor for ResponseHeader {
    fn insert_header_value(&mut self, name: &str, value: &str) -> pingora::Result<()> {
        self.insert_header(name.to_string(), value.to_string())
    }

    fn append_header_value(&mut self, name: &str, value: &str) -> pingora::Result<()> {
        self.append_header(name.to_string(), value.to_string())
            .map(|_| ())
    }

    fn remove_header_value(&mut self, name: &str) {
        self.remove_header(name);
    }
}
