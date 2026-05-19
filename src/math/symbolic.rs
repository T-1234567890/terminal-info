use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::ast::{BinaryOp, Diagnostic, Expr, MathDocument, Statement, UnaryOp, format_number};
use super::graph::{DependencyGraph, build_dependency_graph};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolveOptions {
    pub precision: usize,
    pub include_steps: bool,
    pub include_trace: bool,
    pub debug: bool,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            precision: 6,
            include_steps: false,
            include_trace: false,
            debug: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolveOutput {
    pub status: String,
    pub source_format: String,
    pub results: Vec<MathResult>,
    pub variables: Vec<VariableSummary>,
    pub diagnostics: Vec<Diagnostic>,
    pub steps: Vec<String>,
    pub trace: Vec<String>,
    pub graph: DependencyGraph,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MathResult {
    pub kind: String,
    pub symbol: Option<String>,
    pub expression: String,
    pub value: Option<String>,
    pub latex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VariableSummary {
    pub name: String,
    pub depends_on: Vec<String>,
}

pub fn solve_document(document: &MathDocument, options: &SolveOptions) -> SolveOutput {
    let graph = build_dependency_graph(document);
    let mut diagnostics = document.diagnostics.clone();
    for cycle in &graph.cycles {
        diagnostics.push(Diagnostic::warning(format!(
            "Dependency cycle detected: {cycle}"
        )));
    }

    let mut env: HashMap<String, Expr> = HashMap::new();
    let mut results = Vec::new();
    let mut steps = Vec::new();
    let mut trace = Vec::new();

    for statement in &document.statements {
        match statement {
            Statement::Assignment { name, expr } => {
                let substituted = substitute(expr, &env);
                let simplified = simplify(substituted.clone(), &mut diagnostics);
                if options.include_trace {
                    trace.push(format!("{name}: {expr} -> {substituted} -> {simplified}"));
                }
                env.insert(name.clone(), simplified.clone());
                if options.include_steps {
                    steps.push(format!("Assigned {name} from {expr}."));
                }
                results.push(MathResult {
                    kind: "assignment".to_string(),
                    symbol: Some(name.clone()),
                    expression: expr.to_string(),
                    value: Some(expr_value(&simplified, options.precision)),
                    latex: format!("{name} = {}", simplified.to_latex()),
                });
            }
            Statement::Equation { left, right } => {
                let left = simplify(substitute(left, &env), &mut diagnostics);
                let right = simplify(substitute(right, &env), &mut diagnostics);
                let solved = solve_linear_equation(&left, &right);
                let (value, latex) = match solved {
                    Some((name, value)) => {
                        if options.include_steps {
                            steps.push(format!(
                                "Solved linear equation for {name}: {name} = {}.",
                                format_precision(value, options.precision)
                            ));
                        }
                        (
                            Some(format!(
                                "{name} = {}",
                                format_precision(value, options.precision)
                            )),
                            format!("{name} = {}", format_precision(value, options.precision)),
                        )
                    }
                    None => {
                        diagnostics.push(Diagnostic::unsupported(format!(
                            "Unsupported equation form: {left} = {right}"
                        )));
                        (None, format!("{} = {}", left.to_latex(), right.to_latex()))
                    }
                };
                results.push(MathResult {
                    kind: "equation".to_string(),
                    symbol: None,
                    expression: format!("{left} = {right}"),
                    value,
                    latex,
                });
            }
            Statement::Expression(expr) => {
                let substituted = substitute(expr, &env);
                let simplified = simplify(substituted.clone(), &mut diagnostics);
                if options.include_trace {
                    trace.push(format!("{expr} -> {substituted} -> {simplified}"));
                }
                results.push(MathResult {
                    kind: "expression".to_string(),
                    symbol: None,
                    expression: expr.to_string(),
                    value: Some(expr_value(&simplified, options.precision)),
                    latex: simplified.to_latex(),
                });
            }
        }
    }

    let variables = graph
        .variables
        .iter()
        .map(|variable| VariableSummary {
            name: variable.name.clone(),
            depends_on: variable.depends_on.clone(),
        })
        .collect::<Vec<_>>();
    let status = if diagnostics
        .iter()
        .any(|diag| matches!(diag.level, super::ast::DiagnosticLevel::Error))
    {
        "error"
    } else if diagnostics
        .iter()
        .any(|diag| matches!(diag.level, super::ast::DiagnosticLevel::Unsupported))
    {
        "partial"
    } else {
        "solved"
    };

    SolveOutput {
        status: status.to_string(),
        source_format: document.source_format.label().to_string(),
        results,
        variables,
        diagnostics,
        steps,
        trace,
        graph,
    }
}

fn substitute(expr: &Expr, env: &HashMap<String, Expr>) -> Expr {
    match expr {
        Expr::Number(_) => expr.clone(),
        Expr::Variable(name) => env.get(name).cloned().unwrap_or_else(|| expr.clone()),
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute(expr, env)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(substitute(left, env)),
            right: Box::new(substitute(right, env)),
        },
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|arg| substitute(arg, env)).collect(),
        },
    }
}

fn simplify(expr: Expr, diagnostics: &mut Vec<Diagnostic>) -> Expr {
    match expr {
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => {
            let expr = simplify(*expr, diagnostics);
            if let Expr::Number(value) = expr {
                Expr::Number(-value)
            } else {
                Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                }
            }
        }
        Expr::Binary { op, left, right } => {
            let left = simplify(*left, diagnostics);
            let right = simplify(*right, diagnostics);
            simplify_binary(op, left, right)
        }
        Expr::Call { name, args } => simplify_call(name, args, diagnostics),
        other => other,
    }
}

fn simplify_binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    if let (Expr::Number(a), Expr::Number(b)) = (&left, &right) {
        return match op {
            BinaryOp::Add => Expr::Number(a + b),
            BinaryOp::Sub => Expr::Number(a - b),
            BinaryOp::Mul => Expr::Number(a * b),
            BinaryOp::Div => Expr::Number(a / b),
            BinaryOp::Pow => Expr::Number(a.powf(*b)),
        };
    }

    match (op, &left, &right) {
        (BinaryOp::Add, _, Expr::Number(0.0)) => left,
        (BinaryOp::Add, Expr::Number(0.0), _) => right,
        (BinaryOp::Sub, _, Expr::Number(0.0)) => left,
        (BinaryOp::Mul, _, Expr::Number(1.0)) => left,
        (BinaryOp::Mul, Expr::Number(1.0), _) => right,
        (BinaryOp::Mul, _, Expr::Number(0.0)) | (BinaryOp::Mul, Expr::Number(0.0), _) => {
            Expr::Number(0.0)
        }
        (BinaryOp::Div, _, Expr::Number(1.0)) => left,
        (BinaryOp::Pow, _, Expr::Number(1.0)) => left,
        (BinaryOp::Pow, _, Expr::Number(0.0)) => Expr::Number(1.0),
        _ => Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}

fn simplify_call(name: String, args: Vec<Expr>, diagnostics: &mut Vec<Diagnostic>) -> Expr {
    let args = args
        .into_iter()
        .map(|arg| simplify(arg, diagnostics))
        .collect::<Vec<_>>();
    match name.as_str() {
        "sin" | "cos" | "tan" | "sqrt" if args.len() == 1 => {
            if let Expr::Number(value) = args[0] {
                let value = match name.as_str() {
                    "sin" => value.sin(),
                    "cos" => value.cos(),
                    "tan" => value.tan(),
                    "sqrt" => value.sqrt(),
                    _ => value,
                };
                Expr::Number(value)
            } else {
                Expr::Call { name, args }
            }
        }
        "diff" | "derivative" if args.len() == 2 => {
            if let Expr::Variable(variable) = &args[1] {
                derivative(&args[0], variable)
                    .map(|expr| simplify(expr, diagnostics))
                    .unwrap_or_else(|| {
                        diagnostics.push(Diagnostic::unsupported(format!(
                            "Unsupported derivative form: {}",
                            args[0]
                        )));
                        Expr::Call { name, args }
                    })
            } else {
                diagnostics.push(Diagnostic::unsupported(
                    "Derivative variable must be a symbol such as x.",
                ));
                Expr::Call { name, args }
            }
        }
        "integral" | "integrate" => {
            diagnostics.push(Diagnostic::unsupported(
                "symbolic integration is not available in this build.",
            ));
            Expr::Call { name, args }
        }
        _ => Expr::Call { name, args },
    }
}

fn derivative(expr: &Expr, variable: &str) -> Option<Expr> {
    match expr {
        Expr::Number(_) => Some(Expr::Number(0.0)),
        Expr::Variable(name) => Some(Expr::Number(if name == variable { 1.0 } else { 0.0 })),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => Some(Expr::Unary {
            op: UnaryOp::Neg,
            expr: Box::new(derivative(expr, variable)?),
        }),
        Expr::Binary { op, left, right } => match op {
            BinaryOp::Add | BinaryOp::Sub => Some(Expr::Binary {
                op: *op,
                left: Box::new(derivative(left, variable)?),
                right: Box::new(derivative(right, variable)?),
            }),
            BinaryOp::Mul => {
                if let Expr::Number(value) = **left {
                    Some(Expr::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(Expr::Number(value)),
                        right: Box::new(derivative(right, variable)?),
                    })
                } else if let Expr::Number(value) = **right {
                    Some(Expr::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(Expr::Number(value)),
                        right: Box::new(derivative(left, variable)?),
                    })
                } else {
                    None
                }
            }
            BinaryOp::Pow => {
                if let (Expr::Variable(name), Expr::Number(power)) = (&**left, &**right) {
                    if name == variable {
                        return Some(Expr::Binary {
                            op: BinaryOp::Mul,
                            left: Box::new(Expr::Number(*power)),
                            right: Box::new(Expr::Binary {
                                op: BinaryOp::Pow,
                                left: Box::new(Expr::Variable(name.clone())),
                                right: Box::new(Expr::Number(power - 1.0)),
                            }),
                        });
                    }
                }
                None
            }
            BinaryOp::Div => None,
        },
        Expr::Call { .. } => None,
    }
}

fn solve_linear_equation(left: &Expr, right: &Expr) -> Option<(String, f64)> {
    let diff = Expr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(left.clone()),
        right: Box::new(right.clone()),
    };
    let mut vars = BTreeSet::new();
    diff.collect_vars(&mut vars);
    if vars.len() != 1 {
        return None;
    }
    let var = vars.into_iter().next()?;
    let (a, b) = linear_coeff(&diff, &var)?;
    if a.abs() < 1e-12 {
        return None;
    }
    Some((var, -b / a))
}

fn linear_coeff(expr: &Expr, variable: &str) -> Option<(f64, f64)> {
    match expr {
        Expr::Number(value) => Some((0.0, *value)),
        Expr::Variable(name) if name == variable => Some((1.0, 0.0)),
        Expr::Variable(_) => None,
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => {
            let (a, b) = linear_coeff(expr, variable)?;
            Some((-a, -b))
        }
        Expr::Binary { op, left, right } => {
            let (la, lb) = linear_coeff(left, variable)?;
            let (ra, rb) = linear_coeff(right, variable)?;
            match op {
                BinaryOp::Add => Some((la + ra, lb + rb)),
                BinaryOp::Sub => Some((la - ra, lb - rb)),
                BinaryOp::Mul if la == 0.0 => Some((lb * ra, lb * rb)),
                BinaryOp::Mul if ra == 0.0 => Some((rb * la, rb * lb)),
                BinaryOp::Div if ra == 0.0 && rb != 0.0 => Some((la / rb, lb / rb)),
                _ => None,
            }
        }
        Expr::Call { .. } => None,
    }
}

fn expr_value(expr: &Expr, precision: usize) -> String {
    match expr {
        Expr::Number(value) => format_precision(*value, precision),
        _ => expr.to_string(),
    }
}

fn format_precision(value: f64, precision: usize) -> String {
    if precision == 0 || (value.fract().abs() < 1e-10) {
        format_number(value)
    } else {
        format!("{value:.precision$}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::ast::SourceFormat;
    use crate::math::parser::parse_document;

    #[test]
    fn evaluates_assignments_with_substitution() {
        let doc = parse_document("a=5\nb=a^2+3", SourceFormat::Math).unwrap();
        let output = solve_document(&doc, &SolveOptions::default());
        assert_eq!(output.results[1].value.as_deref(), Some("28"));
    }

    #[test]
    fn solves_linear_equation() {
        let doc = parse_document("x + 2 = 5", SourceFormat::Math).unwrap();
        let output = solve_document(&doc, &SolveOptions::default());
        assert_eq!(output.results[0].value.as_deref(), Some("x = 3"));
    }

    #[test]
    fn computes_simple_derivative() {
        let doc = parse_document("diff(x^2, x)", SourceFormat::Math).unwrap();
        let output = solve_document(&doc, &SolveOptions::default());
        assert_eq!(output.results[0].value.as_deref(), Some("(2 * x)"));
    }
}
