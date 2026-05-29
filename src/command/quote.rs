pub struct QuoteAwareChars<'a> {
    inner: std::str::CharIndices<'a>,
    in_single: bool,
    in_double: bool,
}

impl<'a> QuoteAwareChars<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            inner: s.char_indices(),
            in_single: false,
            in_double: false,
        }
    }
}

impl Iterator for QuoteAwareChars<'_> {
    type Item = (usize, char, bool);

    fn next(&mut self) -> Option<Self::Item> {
        let (i, ch) = self.inner.next()?;
        match ch {
            '\'' if !self.in_double => self.in_single = !self.in_single,
            '"' if !self.in_single => self.in_double = !self.in_double,
            _ => {}
        }
        let quoted = self.in_single || self.in_double;
        Some((i, ch, quoted))
    }
}

pub fn quotes_balanced(s: &str) -> bool {
    let mut qac = QuoteAwareChars::new(s);
    while qac.next().is_some() {}
    !qac.in_single && !qac.in_double
}

pub fn strip_quoted_sections(cmd: &str) -> String {
    QuoteAwareChars::new(cmd)
        .filter(|&(_, ch, quoted)| !quoted && ch != '\'' && ch != '"')
        .map(|(_, ch, _)| ch)
        .collect()
}

pub fn shell_split(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut qac = QuoteAwareChars::new(cmd);

    while let Some((_, ch, _)) = qac.next() {
        let is_unquoted_space = ch == ' ' && !qac.in_single && !qac.in_double;
        let is_delimiter = (ch == '\'' && !qac.in_double) || (ch == '"' && !qac.in_single);

        if is_unquoted_space {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }
        } else if !is_delimiter {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

pub fn unquoted_contains_any(cmd: &str, patterns: &[&str]) -> bool {
    let unquoted = strip_quoted_sections(cmd);
    patterns.iter().any(|p| unquoted.contains(p))
}
