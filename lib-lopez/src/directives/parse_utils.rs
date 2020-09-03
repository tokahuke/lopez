use nom::error::ErrorKind;
use nom::IResult;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    line: usize,
    column: usize,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line + 1, self.column + 1)
    }
}

impl Position {
    fn of(text: &str, fragment: &str) -> Position {
        let fragment_pos = text.len() - fragment.len();
        let mut line = 0;
        let mut column = 0;

        for ch in text[..fragment_pos].chars() {
            if ch == '\n' {
                line += 1;
                column = 0;
            } else if ch != '\r' {
                column += 1;
            }
        }

        Position { line, column }
    }
}

#[derive(Debug)]
pub struct ParseError {
    position: Position,
    hint: String,
    message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {} ({:?}): {}", self.position, self.hint, self.message)
    }
}

impl ParseError {
    pub fn new(text: &str, err: nom::Err<(&str, ErrorKind)>) -> ParseError {
        match err {
            nom::Err::Error((fragment, error_kind)) | nom::Err::Failure((fragment, error_kind)) => {
                ParseError {
                    position: Position::of(text, fragment),
                    hint: fragment.lines().map(str::to_owned).next().unwrap_or_default().chars().take(10).collect::<String>() + "...",
                    message: error_kind.description().to_owned(),
                }
            }
            nom::Err::Incomplete(_) => panic!("incomplete variant no accepted"),
        }
    }

    pub fn map_iresult<T>(text: &str, iresult: IResult<&str, T>) -> Result<T, ParseError> {
        match iresult {
            Ok((_left_over, result)) => Ok(result),
            Err(err) => Err(ParseError::new(text, err)),
        }
    }
}
