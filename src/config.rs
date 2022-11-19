use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct RedactedString(pub String);

impl RedactedString {
    pub fn new(inner: impl Into<String>) -> Self {
        Self(inner.into())
    }
}

/// We manually implement this to only print a few characters of the secret.
impl fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            r#"RedactedString("{}***")"#,
            &self.0.chars().take(4).collect::<String>()
        )
    }
}
