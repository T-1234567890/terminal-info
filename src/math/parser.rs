use serde_json::Value;

use super::ast::{BinaryOp, Diagnostic, Expr, MathDocument, SourceFormat, Statement, UnaryOp};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Eq,
    LParen,
    RParen,
    Comma,
}

pub fn parse_document(content: &str, format: SourceFormat) -> Result<MathDocument, String> {
    match format {
        SourceFormat::Json => parse_json_document(content),
        SourceFormat::Latex => parse_text_document(&extract_latex_math(content), format),
        _ => parse_text_document(content, format),
    }
}

fn parse_json_document(content: &str) -> Result<MathDocument, String> {
    let value = serde_json::from_str::<Value>(content)
        .map_err(|err| format!("Failed to parse math JSON input: {err}"))?;
    let mut lines = Vec::new();
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(text) = item.as_str() {
                    lines.push(text.to_string());
                }
            }
        }
        Value::Object(map) => {
            if let Some(statements) = map.get("statements").and_then(Value::as_array) {
                for item in statements {
                    if let Some(text) = item.as_str() {
                        lines.push(text.to_string());
                    }
                }
            } else {
                for (key, value) in map {
                    if let Some(text) = value.as_str() {
                        lines.push(format!("{key} = {text}"));
                    } else if value.is_number() {
                        lines.push(format!("{key} = {value}"));
                    }
                }
            }
        }
        Value::String(text) => lines.push(text),
        _ => return Err("Math JSON input must be a string, array, or object.".to_string()),
    }
    parse_text_document(&lines.join("\n"), SourceFormat::Json)
}

fn parse_text_document(content: &str, format: SourceFormat) -> Result<MathDocument, String> {
    let mut statements = Vec::new();
    let mut diagnostics = Vec::new();

    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.trim_matches('$').trim();
        match parse_statement(line) {
            Ok(statement) => statements.push(statement),
            Err(err) => diagnostics.push(Diagnostic::warning(format!("line {}: {err}", index + 1))),
        }
    }

    if statements.is_empty() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic::warning("No math statements found."));
    }

    Ok(MathDocument {
        statements,
        source_format: format,
        diagnostics,
    })
}

fn extract_latex_math(content: &str) -> String {
    let mut extracted = Vec::new();
    let mut in_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("$$")
            || trimmed.starts_with("\\[")
            || trimmed.contains("\\begin{equation")
        {
            in_block = true;
            let cleaned = clean_latex_line(trimmed);
            if !cleaned.is_empty() {
                extracted.push(cleaned);
            }
            continue;
        }
        if trimmed.ends_with("$$") || trimmed.ends_with("\\]") || trimmed.contains("\\end{equation")
        {
            let cleaned = clean_latex_line(trimmed);
            if !cleaned.is_empty() {
                extracted.push(cleaned);
            }
            in_block = false;
            continue;
        }
        if in_block || trimmed.contains('$') {
            let cleaned = clean_latex_line(trimmed);
            if !cleaned.is_empty() {
                extracted.push(cleaned);
            }
        }
    }
    if extracted.is_empty() {
        content
            .lines()
            .map(clean_latex_line)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        extracted.join("\n")
    }
}

fn clean_latex_line(line: &str) -> String {
    line.replace("$$", "")
        .replace("\\[", "")
        .replace("\\]", "")
        .replace("\\begin{equation}", "")
        .replace("\\end{equation}", "")
        .replace("\\cdot", "*")
        .replace("\\times", "*")
        .replace("\\left", "")
        .replace("\\right", "")
        .replace('\\', "")
        .replace('$', "")
        .trim()
        .to_string()
}

fn parse_statement(input: &str) -> Result<Statement, String> {
    let tokens = tokenize(input)?;
    if tokens
        .iter()
        .filter(|token| matches!(token, Token::Eq))
        .count()
        == 1
    {
        if let [Token::Ident(name), Token::Eq, rest @ ..] = tokens.as_slice() {
            let expr = parse_expr_tokens(rest)?;
            return Ok(Statement::Assignment {
                name: name.clone(),
                expr,
            });
        }
        let eq_pos = tokens
            .iter()
            .position(|token| matches!(token, Token::Eq))
            .ok_or_else(|| "Missing equation operator.".to_string())?;
        let left = parse_expr_tokens(&tokens[..eq_pos])?;
        let right = parse_expr_tokens(&tokens[eq_pos + 1..])?;
        return Ok(Statement::Equation { left, right });
    }
    Ok(Statement::Expression(parse_expr_tokens(&tokens)?))
}

fn parse_expr_tokens(tokens: &[Token]) -> Result<Expr, String> {
    if tokens.is_empty() {
        return Err("Expected expression.".to_string());
    }
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_expr(0)?;
    if parser.pos != tokens.len() {
        return Err("Unexpected trailing tokens.".to_string());
    }
    Ok(expr)
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        match ch {
            ' ' | '\t' | '\r' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut number = String::new();
                while let Some(ch) = chars.peek().copied() {
                    if ch.is_ascii_digit() || ch == '.' {
                        number.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let parsed = number
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid number '{number}'."))?;
                tokens.push(Token::Number(parsed));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(ch) = chars.peek().copied() {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        ident.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(ident));
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Token::Slash);
            }
            '^' => {
                chars.next();
                tokens.push(Token::Caret);
            }
            '=' => {
                chars.next();
                tokens.push(Token::Eq);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            _ => return Err(format!("Unexpected character '{ch}'.")),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl Parser<'_> {
    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let token = self
            .next()
            .cloned()
            .ok_or_else(|| "Expected expression.".to_string())?;
        let mut lhs = match token {
            Token::Number(value) => Expr::Number(value),
            Token::Ident(name) => {
                if self.peek() == Some(&Token::LParen) {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if self.peek() == Some(&Token::Comma) {
                                self.next();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                    Expr::Call { name, args }
                } else {
                    Expr::Variable(name)
                }
            }
            Token::Minus => {
                let expr = self.parse_expr(9)?;
                Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                }
            }
            Token::LParen => {
                let expr = self.parse_expr(0)?;
                self.expect(Token::RParen)?;
                expr
            }
            _ => return Err("Expected expression.".to_string()),
        };

        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Sub,
                Some(Token::Star) => BinaryOp::Mul,
                Some(Token::Slash) => BinaryOp::Div,
                Some(Token::Caret) => BinaryOp::Pow,
                _ => break,
            };
            let (left_bp, right_bp) = binding_power(op);
            if left_bp < min_bp {
                break;
            }
            self.next();
            let rhs = self.parse_expr(right_bp)?;
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        self.pos += usize::from(token.is_some());
        token
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if self.next() == Some(&expected) {
            Ok(())
        } else {
            Err("Unexpected token.".to_string())
        }
    }
}

fn binding_power(op: BinaryOp) -> (u8, u8) {
    match op {
        BinaryOp::Add | BinaryOp::Sub => (1, 2),
        BinaryOp::Mul | BinaryOp::Div => (3, 4),
        BinaryOp::Pow => (7, 6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_respects_precedence() {
        let doc = parse_document("b = a^2 + 3 * 4", SourceFormat::Math).unwrap();
        assert_eq!(doc.statements[0].to_string(), "b = ((a ^ 2) + (3 * 4))");
    }

    #[test]
    fn parses_json_object() {
        let doc = parse_document(r#"{"a":5,"b":"a^2+3"}"#, SourceFormat::Json).unwrap();
        assert_eq!(doc.statements.len(), 2);
    }

    #[test]
    fn extracts_simple_latex() {
        let doc = parse_document(r"\[x + 2 = 5\]", SourceFormat::Latex).unwrap();
        assert!(matches!(doc.statements[0], Statement::Equation { .. }));
    }
}
