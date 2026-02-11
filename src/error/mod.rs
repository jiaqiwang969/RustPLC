use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlcError {
    #[error("第 {line} 行错误: {message}")]
    Parse { line: usize, message: String },
    #[error("第 {line} 行语义错误: {message}")]
    Semantic { line: usize, message: String },
}

impl PlcError {
    pub fn parse(line: usize, message: impl Into<String>) -> Self {
        Self::Parse {
            line,
            message: message.into(),
        }
    }

    pub fn semantic(line: usize, message: impl Into<String>) -> Self {
        Self::Semantic {
            line,
            message: message.into(),
        }
    }

    pub fn line(&self) -> usize {
        match self {
            Self::Parse { line, .. } | Self::Semantic { line, .. } => *line,
        }
    }
}
