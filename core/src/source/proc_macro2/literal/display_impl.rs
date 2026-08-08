use super::*;

impl std::fmt::Display for Bool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.value { "true" } else { "false" })
    }
}

impl std::fmt::Display for ByteChar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.value {
            b'\n' => write!(f, "b'\\n'"),
            b'\t' => write!(f, "b'\\t'"),
            b'\r' => write!(f, "b'\\r'"),
            b'\\' => write!(f, "b'\\\\'"),
            b'\'' => write!(f, "b'\\''"),
            0 => write!(f, "b'\\0'"),
            b if b.is_ascii_graphic() || b == b' ' => write!(f, "b'{}'", b as char),
            b => write!(f, "b'\\x{:02x}'", b),
        }
    }
}

impl std::fmt::Display for Char {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.value {
            '\n' => write!(f, "'\\n'"),
            '\t' => write!(f, "'\\t'"),
            '\r' => write!(f, "'\\r'"),
            '\\' => write!(f, "'\\\\'"),
            '\'' => write!(f, "'\\''"),
            '\0' => write!(f, "'\\0'"),
            c => write!(f, "'{}'", c),
        }
    }
}

impl std::fmt::Display for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.suffix.as_deref().unwrap_or(""))
    }
}

impl std::fmt::Display for Float {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.suffix.as_deref().unwrap_or(""))
    }
}

impl std::fmt::Display for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\"{}\"",
            self.value.replace('\\', "\\\\").replace('"', "\\\"")
        )
    }
}

fn raw(
    f: &mut std::fmt::Formatter<'_>,
    prefix: &str,
    value: impl std::fmt::Display,
    hash_count: usize,
) -> std::fmt::Result {
    let hashes = "#".repeat(hash_count);
    write!(f, "{}{}\"{}\"{}", prefix, hashes, value, hashes)
}

impl std::fmt::Display for StrRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        raw(f, "r", &self.value, self.hash_count)
    }
}

impl std::fmt::Display for ByteStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "b\"")?;
        for &byte in &self.value {
            match byte {
                b'\n' => write!(f, "\\n")?,
                b'\t' => write!(f, "\\t")?,
                b'\r' => write!(f, "\\r")?,
                b'\\' => write!(f, "\\\\")?,
                b'"' => write!(f, "\\\"")?,
                b if b.is_ascii_graphic() || b == b' ' => write!(f, "{}", b as char)?,
                b => write!(f, "\\x{:02x}", b)?,
            }
        }
        write!(f, "\"")
    }
}

impl std::fmt::Display for ByteStrRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        raw(
            f,
            "br",
            String::from_utf8_lossy(&self.value),
            self.hash_count,
        )
    }
}

impl std::fmt::Display for CStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "c\"{}\"",
            self.value.replace('\\', "\\\\").replace('"', "\\\"")
        )
    }
}

impl std::fmt::Display for CStrRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        raw(f, "cr", &self.value, self.hash_count)
    }
}
