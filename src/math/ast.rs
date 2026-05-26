use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Math,
    Text,
    Json,
    Latex,
    Unknown,
}

impl SourceFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Math => "math",
            Self::Text => "text",
            Self::Json => "json",
            Self::Latex => "latex",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MathDocument {
    pub statements: Vec<Statement>,
    pub source_format: SourceFormat,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

impl Diagnostic {
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            message: message.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Unsupported,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Warning,
    Error,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Statement {
    Assignment { name: String, expr: Expr },
    Equation { left: Expr, right: Expr },
    Assumption { variable: String, condition: String },
    Expression(Expr),
}

impl Statement {
    pub fn to_latex(&self) -> String {
        match self {
            Self::Assignment { name, expr } => format!("{name} = {}", expr.to_latex()),
            Self::Equation { left, right } => {
                format!("{} = {}", left.to_latex(), right.to_latex())
            }
            Self::Assumption {
                variable,
                condition,
            } => format!("\\operatorname{{assume}}({variable}: {condition})"),
            Self::Expression(expr) => expr.to_latex(),
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assignment { name, expr } => write!(f, "{name} = {expr}"),
            Self::Equation { left, right } => write!(f, "{left} = {right}"),
            Self::Assumption {
                variable,
                condition,
            } => write!(f, "assume {variable} {condition}"),
            Self::Expression(expr) => write!(f, "{expr}"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Expr {
    Number(f64),
    Variable(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Quantity {
        value: Box<Expr>,
        unit: String,
    },
}

impl Expr {
    pub fn collect_vars(&self, vars: &mut BTreeSet<String>) {
        match self {
            Self::Number(_) => {}
            Self::Variable(name) => {
                vars.insert(name.clone());
            }
            Self::Unary { expr, .. } => expr.collect_vars(vars),
            Self::Binary { left, right, .. } => {
                left.collect_vars(vars);
                right.collect_vars(vars);
            }
            Self::Call { args, .. } => {
                for arg in args {
                    arg.collect_vars(vars);
                }
            }
            Self::Quantity { value, .. } => value.collect_vars(vars),
        }
    }

    pub fn to_latex(&self) -> String {
        match self {
            Self::Number(value) => format_number(*value),
            Self::Variable(name) => name.clone(),
            Self::Unary { op, expr } => match op {
                UnaryOp::Neg => format!("-{}", expr.to_latex()),
            },
            Self::Binary { op, left, right } => match op {
                BinaryOp::Add => format!("{} + {}", left.to_latex(), right.to_latex()),
                BinaryOp::Sub => format!("{} - {}", left.to_latex(), right.to_latex()),
                BinaryOp::Mul => format!("{} \\cdot {}", left.to_latex(), right.to_latex()),
                BinaryOp::Div => format!("\\frac{{{}}}{{{}}}", left.to_latex(), right.to_latex()),
                BinaryOp::Pow => format!("{}^{{{}}}", left.to_latex(), right.to_latex()),
            },
            Self::Call { name, args } => {
                let args = args
                    .iter()
                    .map(Expr::to_latex)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("\\operatorname{{{name}}}({args})")
            }
            Self::Quantity { value, unit } => format!("{}\\,{}", value.to_latex(), unit),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => write!(f, "{}", format_number(*value)),
            Self::Variable(name) => write!(f, "{name}"),
            Self::Unary { op, expr } => match op {
                UnaryOp::Neg => write!(f, "-{expr}"),
            },
            Self::Binary { op, left, right } => write!(f, "({left} {} {right})", op.label()),
            Self::Call { name, args } => {
                let args = args
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{name}({args})")
            }
            Self::Quantity { value, unit } => write!(f, "{value} {unit}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    Neg,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl BinaryOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Pow => "^",
        }
    }
}

pub fn format_number(value: f64) -> String {
    if value.is_finite() && (value.fract().abs() < 1e-10) {
        format!("{}", value as i64)
    } else {
        format!("{value:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
