#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    UnexpectedToken { expected: String, got: String },
    UnexpectedEof,
    AlreadyConsumed(String),
    NotDefined(String),
    UnknownType(String),
    ImportNotFound(String),
    UnsupportedFeature(String),
    AlreadyDefined(String),
}

impl ErrorKind {
    fn code(&self) -> &'static str {
        match self {
            ErrorKind::UnexpectedToken { .. } => "E0001",
            ErrorKind::UnexpectedEof => "E0002",
            ErrorKind::AlreadyConsumed(_) => "E0003",
            ErrorKind::NotDefined(_) => "E0004",
            ErrorKind::UnknownType(_) => "E0005",
            ErrorKind::ImportNotFound(_) => "E0006",
            ErrorKind::UnsupportedFeature(_) => "E0007",
            ErrorKind::AlreadyDefined(_) => "E0008",
        }
    }

    fn message(&self) -> String {
        match self {
            ErrorKind::UnexpectedToken { expected, got } => {
                format!("expected {} but got {}", expected, got)
            }
            ErrorKind::UnexpectedEof => "unexpected end of file".to_string(),
            ErrorKind::AlreadyConsumed(name) => format!("use of consumed value `{}`", name),
            ErrorKind::NotDefined(name) => format!("cannot find value `{}` in this scope", name),
            ErrorKind::UnknownType(name) => format!("cannot find type `{}` in this scope", name),
            ErrorKind::ImportNotFound(path) => format!("cannot find file `{}`", path),
            ErrorKind::UnsupportedFeature(msg) => format!("not yet supported: {}", msg),
            ErrorKind::AlreadyDefined(name) => {
                format!("'{}' is already defined in this scope", name)
            }
        }
    }

    fn note(&self) -> Option<String> {
        match self {
            ErrorKind::AlreadyConsumed(name) => Some(format!(
                "`{}` was consumed here and cannot be used again",
                name
            )),
            ErrorKind::UnknownType(name) => Some(format!(
                "make sure the .dt file defining `{}` is imported",
                name
            )),
            ErrorKind::ImportNotFound(path) => Some(format!(
                "check that `{}` exists relative to your working directory",
                path
            )),
            _ => None,
        }
    }

    fn token(&self) -> &str {
        match self {
            ErrorKind::AlreadyConsumed(n) => n,
            ErrorKind::NotDefined(n) => n,
            ErrorKind::UnknownType(n) => n,
            ErrorKind::ImportNotFound(n) => n,
            ErrorKind::UnsupportedFeature(n) => n,
            ErrorKind::AlreadyDefined(n) => n,
            ErrorKind::UnexpectedToken { got, .. } => got,
            _ => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub kind: ErrorKind,
    pub location: Option<SourceLocation>,
    pub source_line: Option<String>,
}

impl CompileError {
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            location: None,
            source_line: None,
        }
    }

    pub fn at(mut self, file: &str, line: usize, col: usize, len: usize) -> Self {
        self.location = Some(SourceLocation {
            file: file.to_string(),
            line,
            col,
            len,
        });
        self
    }

    pub fn with_source(mut self, source_line: String) -> Self {
        self.source_line = Some(source_line);
        self
    }

    pub fn report(&self) {
        eprintln!(
            "\x1b[31merror[{}]\x1b[0m: {}",
            self.kind.code(),
            self.kind.message()
        );

        if let Some(loc) = &self.location {
            eprintln!("  \x1b[36m-->\x1b[0m {}:{}:{}", loc.file, loc.line, loc.col);

            if let Some(src) = &self.source_line {
                let line_str = loc.line.to_string();
                let padding = " ".repeat(line_str.len());
                eprintln!("  \x1b[36m{} |\x1b[0m", padding);
                eprintln!("  \x1b[36m{} |\x1b[0m {}", line_str, src);
                let caret = "^".repeat(loc.len);
                let caret_pad = " ".repeat(loc.col.saturating_sub(1));
                eprintln!(
                    "  \x1b[36m{} |\x1b[0m {}\x1b[31m{}\x1b[0m",
                    padding, caret_pad, caret
                );
            }
        }

        if let Some(note) = self.kind.note() {
            eprintln!("  \x1b[33mnote\x1b[0m: {}", note);
        }

        eprintln!();
    }
}

pub type CompileResult<T> = Result<T, CompileError>;

pub struct ErrorReporter {
    errors: Vec<CompileError>,
    file: String,
    source_lines: Vec<String>,
}

impl ErrorReporter {
    pub fn new(file: &str) -> Self {
        let source_lines = std::fs::read_to_string(file)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect();

        Self {
            errors: Vec::new(),
            file: file.to_string(),
            source_lines,
        }
    }

    fn find_whole_word(src: &str, token: &str) -> Option<usize> {
        if token.is_empty() {
            return None;
        }
        src.match_indices(token)
            .find(|(i, _)| {
                let before = i.checked_sub(1).and_then(|i| src.chars().nth(i));
                let after = src.chars().nth(i + token.len());
                !before
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false)
                    && !after
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false)
            })
            .map(|(i, _)| i + 1)
    }

    pub fn error(&mut self, kind: ErrorKind, line: usize, _col: usize, _len: usize) {
        let source_line = self.source_lines.get(line.saturating_sub(1)).cloned();
        let token = kind.token();
        let len = token.len().max(1);
        let col = source_line
            .as_deref()
            .and_then(|src| Self::find_whole_word(src, token))
            .unwrap_or(0);
        let mut err = CompileError::new(kind).at(&self.file, line, col, len);
        if let Some(src) = source_line {
            err = err.with_source(src);
        }
        self.errors.push(err);
    }

    pub fn error_no_loc(&mut self, kind: ErrorKind) {
        self.errors.push(CompileError::new(kind));
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn report_all(&self) {
        for e in &self.errors {
            e.report();
        }
        eprintln!(
            "\x1b[31merror\x1b[0m: aborting due to {} previous error{}",
            self.errors.len(),
            if self.errors.len() == 1 { "" } else { "s" }
        );
    }

    pub fn fatal_if_any(&self) {
        if self.has_errors() {
            self.report_all();
            std::process::exit(1);
        }
    }
}
