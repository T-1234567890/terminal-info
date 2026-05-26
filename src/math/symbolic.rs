use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::ast::{BinaryOp, Diagnostic, Expr, MathDocument, Statement, UnaryOp, format_number};
use super::graph::{DependencyGraph, build_dependency_graph};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolveOptions {
    pub precision: usize,
    pub solve_for: Option<String>,
    pub include_steps: bool,
    pub include_trace: bool,
    pub debug: bool,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            precision: 6,
            solve_for: None,
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
    pub assumptions: Vec<AssumptionSummary>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssumptionSummary {
    pub variable: String,
    pub condition: String,
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
    let mut assumptions = Vec::new();
    let mut pending_equations: Vec<(usize, Expr, Expr)> = Vec::new();

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
                let solved = solve_single_equation(
                    &left,
                    &right,
                    options.precision,
                    options.solve_for.as_deref(),
                );
                let (value, latex) = match solved {
                    Some((name, value)) => {
                        if options.include_steps {
                            steps.push(format!("Solved equation for {name}: {value}."));
                        }
                        (Some(value.clone()), value)
                    }
                    None => (None, format!("{} = {}", left.to_latex(), right.to_latex())),
                };
                let result_index = results.len();
                results.push(MathResult {
                    kind: "equation".to_string(),
                    symbol: None,
                    expression: format!("{left} = {right}"),
                    value,
                    latex,
                });
                if results[result_index].value.is_none() {
                    pending_equations.push((result_index, left, right));
                }
            }
            Statement::Assumption {
                variable,
                condition,
            } => {
                assumptions.push(AssumptionSummary {
                    variable: variable.clone(),
                    condition: condition.clone(),
                });
                if options.include_steps {
                    steps.push(format!("Recorded assumption: {variable} {condition}."));
                }
                results.push(MathResult {
                    kind: "assumption".to_string(),
                    symbol: Some(variable.clone()),
                    expression: format!("assume {variable} {condition}"),
                    value: Some(format!("{variable} {condition}")),
                    latex: statement.to_latex(),
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

    if !pending_equations.is_empty() {
        if pending_equations.len() >= 2 {
            if let Some(solution) = solve_linear_system(&pending_equations, options.precision) {
                let value = solution
                    .iter()
                    .map(|(name, value)| format!("{name} = {value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let latex = solution
                    .iter()
                    .map(|(name, value)| format!("{name} = {value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let expression = pending_equations
                    .iter()
                    .map(|(_, left, right)| format!("{left} = {right}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                results.push(MathResult {
                    kind: "linear_system".to_string(),
                    symbol: None,
                    expression,
                    value: Some(value.clone()),
                    latex,
                });
                if options.include_steps {
                    steps.push(format!("Solved linear system: {value}."));
                }
            } else {
                for (_, left, right) in &pending_equations {
                    diagnostics.push(Diagnostic::unsupported(format!(
                        "Unsupported equation form: {left} = {right}"
                    )));
                }
            }
        } else {
            for (_, left, right) in &pending_equations {
                diagnostics.push(Diagnostic::unsupported(format!(
                    "Unsupported equation form: {left} = {right}"
                )));
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
        assumptions,
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
        Expr::Quantity { value, unit } => Expr::Quantity {
            value: Box::new(substitute(value, env)),
            unit: unit.clone(),
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
        Expr::Quantity { value, unit } => Expr::Quantity {
            value: Box::new(simplify(*value, diagnostics)),
            unit,
        },
        other => other,
    }
}

fn simplify_binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    if let Some(quantity) = simplify_quantity_binary(op, &left, &right) {
        return quantity;
    }

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
        "ln" | "log" | "exp" if args.len() == 1 => {
            if let Expr::Number(value) = args[0] {
                let value = match name.as_str() {
                    "ln" | "log" => value.ln(),
                    "exp" => value.exp(),
                    _ => value,
                };
                Expr::Number(value)
            } else {
                Expr::Call { name, args }
            }
        }
        "lim" | "limit" if args.len() == 3 => match (&args[1], &args[2]) {
            (Expr::Variable(variable), Expr::Number(target)) => {
                let mut env = HashMap::new();
                env.insert(variable.clone(), Expr::Number(*target));
                let evaluated = simplify(substitute(&args[0], &env), diagnostics);
                if contains_non_finite(&evaluated) {
                    diagnostics.push(Diagnostic::unsupported(format!(
                        "Unsupported limit form: lim({}, {}, {})",
                        args[0], variable, target
                    )));
                    Expr::Call { name, args }
                } else {
                    evaluated
                }
            }
            _ => {
                diagnostics.push(Diagnostic::unsupported(
                    "Limit must look like lim(expr, x, value).",
                ));
                Expr::Call { name, args }
            }
        },
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
        "integral" | "integrate" if args.len() == 2 => {
            if let Expr::Variable(variable) = &args[1] {
                integrate(&args[0], variable)
                    .map(|expr| simplify(expr, diagnostics))
                    .unwrap_or_else(|| {
                        diagnostics.push(Diagnostic::unsupported(format!(
                            "Unsupported integral form: {}",
                            args[0]
                        )));
                        Expr::Call { name, args }
                    })
            } else {
                diagnostics.push(Diagnostic::unsupported(
                    "Integral variable must be a symbol such as x.",
                ));
                Expr::Call { name, args }
            }
        }
        "integral" | "integrate" => {
            diagnostics.push(Diagnostic::unsupported(
                "Integral must look like integrate(expr, x).",
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
                let left_prime = derivative(left, variable)?;
                let right_prime = derivative(right, variable)?;
                Some(add(
                    mul(left_prime, *right.clone()),
                    mul(*left.clone(), right_prime),
                ))
            }
            BinaryOp::Pow => {
                if let Expr::Number(power) = &**right {
                    return Some(mul(
                        mul(
                            Expr::Number(*power),
                            pow(*left.clone(), Expr::Number(power - 1.0)),
                        ),
                        derivative(left, variable)?,
                    ));
                }
                None
            }
            BinaryOp::Div => {
                let numerator = sub(
                    mul(derivative(left, variable)?, *right.clone()),
                    mul(*left.clone(), derivative(right, variable)?),
                );
                Some(div(numerator, pow(*right.clone(), Expr::Number(2.0))))
            }
        },
        Expr::Call { name, args } if args.len() == 1 => {
            let inner = args[0].clone();
            let inner_prime = derivative(&inner, variable)?;
            match name.as_str() {
                "sin" => Some(mul(call("cos", vec![inner]), inner_prime)),
                "cos" => Some(mul(
                    Expr::Unary {
                        op: UnaryOp::Neg,
                        expr: Box::new(call("sin", vec![inner])),
                    },
                    inner_prime,
                )),
                "tan" => Some(mul(
                    div(
                        Expr::Number(1.0),
                        pow(call("cos", vec![inner]), Expr::Number(2.0)),
                    ),
                    inner_prime,
                )),
                "ln" | "log" => Some(div(inner_prime, inner)),
                "exp" => Some(mul(call("exp", vec![inner]), inner_prime)),
                "sqrt" => Some(div(
                    inner_prime,
                    mul(Expr::Number(2.0), call("sqrt", vec![inner])),
                )),
                _ => None,
            }
        }
        Expr::Call { .. } | Expr::Quantity { .. } => None,
    }
}

fn integrate(expr: &Expr, variable: &str) -> Option<Expr> {
    match expr {
        Expr::Number(value) => Some(mul(
            Expr::Number(*value),
            Expr::Variable(variable.to_string()),
        )),
        Expr::Variable(name) if name == variable => Some(div(
            pow(Expr::Variable(name.clone()), Expr::Number(2.0)),
            Expr::Number(2.0),
        )),
        Expr::Variable(_) => Some(mul(expr.clone(), Expr::Variable(variable.to_string()))),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => Some(Expr::Unary {
            op: UnaryOp::Neg,
            expr: Box::new(integrate(expr, variable)?),
        }),
        Expr::Binary { op, left, right } => match op {
            BinaryOp::Add | BinaryOp::Sub => Some(Expr::Binary {
                op: *op,
                left: Box::new(integrate(left, variable)?),
                right: Box::new(integrate(right, variable)?),
            }),
            BinaryOp::Mul => {
                if let Expr::Number(value) = **left {
                    Some(mul(Expr::Number(value), integrate(right, variable)?))
                } else if let Expr::Number(value) = **right {
                    Some(mul(Expr::Number(value), integrate(left, variable)?))
                } else {
                    None
                }
            }
            BinaryOp::Pow => {
                if let (Expr::Variable(name), Expr::Number(power)) = (&**left, &**right)
                    && name == variable
                    && (*power + 1.0).abs() > 1e-12
                {
                    let next = power + 1.0;
                    return Some(div(
                        pow(Expr::Variable(name.clone()), Expr::Number(next)),
                        Expr::Number(next),
                    ));
                }
                None
            }
            _ => None,
        },
        Expr::Call { name, args } if args.len() == 1 => {
            if args[0] != Expr::Variable(variable.to_string()) {
                return None;
            }
            match name.as_str() {
                "sin" => Some(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(call("cos", vec![args[0].clone()])),
                }),
                "cos" => Some(call("sin", vec![args[0].clone()])),
                "exp" => Some(call("exp", vec![args[0].clone()])),
                _ => None,
            }
        }
        Expr::Call { .. } | Expr::Quantity { .. } => None,
    }
}

fn solve_single_equation(
    left: &Expr,
    right: &Expr,
    precision: usize,
    solve_for: Option<&str>,
) -> Option<(String, String)> {
    if let Some(variable) = solve_for {
        if let Some(value) = solve_for_variable(left, right, variable, precision) {
            return Some((variable.to_string(), format!("{variable} = {value}")));
        }
        if let Some((name, roots)) = solve_quadratic_equation(left, right, precision)
            && name == variable
        {
            let value = roots
                .iter()
                .enumerate()
                .map(|(index, root)| format!("{name}{} = {root}", index + 1))
                .collect::<Vec<_>>()
                .join(", ");
            return Some((name, value));
        }
        return None;
    }
    if let Some((name, value)) = solve_linear_equation(left, right) {
        let formatted = format_precision(value, precision);
        return Some((name.clone(), format!("{name} = {formatted}")));
    }
    if let Some((name, roots)) = solve_quadratic_equation(left, right, precision) {
        let value = roots
            .iter()
            .enumerate()
            .map(|(index, root)| format!("{name}{} = {root}", index + 1))
            .collect::<Vec<_>>()
            .join(", ");
        return Some((name, value));
    }
    None
}

fn solve_for_variable(
    left: &Expr,
    right: &Expr,
    variable: &str,
    precision: usize,
) -> Option<String> {
    let diff = sub(left.clone(), right.clone());
    let (coeffs, constant) = linear_coeffs(&diff)?;
    let target_coeff = *coeffs.get(variable)?;
    if target_coeff.abs() < 1e-12 {
        return None;
    }
    let mut rhs = Expr::Number(-constant);
    for (name, coeff) in coeffs {
        if name == variable || coeff.abs() < 1e-12 {
            continue;
        }
        rhs = sub(rhs, mul(Expr::Number(coeff), Expr::Variable(name)));
    }
    let solved = simplify(div(rhs, Expr::Number(target_coeff)), &mut Vec::new());
    Some(expr_value(&solved, precision))
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

fn solve_quadratic_equation(
    left: &Expr,
    right: &Expr,
    precision: usize,
) -> Option<(String, Vec<String>)> {
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
    let coeffs = polynomial_coeff(&diff, &var)?;
    let a = *coeffs.get(&2).unwrap_or(&0.0);
    let b = *coeffs.get(&1).unwrap_or(&0.0);
    let c = *coeffs.get(&0).unwrap_or(&0.0);
    if a.abs() < 1e-12 {
        return None;
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < -1e-12 {
        return None;
    }
    if discriminant.abs() < 1e-12 {
        return Some((var, vec![format_precision(-b / (2.0 * a), precision)]));
    }
    let sqrt = discriminant.sqrt();
    Some((
        var,
        vec![
            format_precision((-b + sqrt) / (2.0 * a), precision),
            format_precision((-b - sqrt) / (2.0 * a), precision),
        ],
    ))
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
        Expr::Call { .. } | Expr::Quantity { .. } => None,
    }
}

fn solve_linear_system(
    equations: &[(usize, Expr, Expr)],
    precision: usize,
) -> Option<Vec<(String, String)>> {
    let mut variables = BTreeSet::new();
    let mut rows = Vec::new();
    for (_, left, right) in equations {
        let diff = sub(left.clone(), right.clone());
        let (coeffs, constant) = linear_coeffs(&diff)?;
        for variable in coeffs.keys() {
            variables.insert(variable.clone());
        }
        rows.push((coeffs, -constant));
    }
    if variables.len() != equations.len() {
        return None;
    }
    let variables = variables.into_iter().collect::<Vec<_>>();
    let mut matrix = rows
        .into_iter()
        .map(|(coeffs, rhs)| {
            let mut row = variables
                .iter()
                .map(|name| *coeffs.get(name).unwrap_or(&0.0))
                .collect::<Vec<_>>();
            row.push(rhs);
            row
        })
        .collect::<Vec<_>>();
    let solution = gaussian_elimination(&mut matrix)?;
    Some(
        variables
            .into_iter()
            .zip(solution)
            .map(|(name, value)| (name, format_precision(value, precision)))
            .collect(),
    )
}

fn gaussian_elimination(matrix: &mut [Vec<f64>]) -> Option<Vec<f64>> {
    let n = matrix.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&a, &b| {
            matrix[a][col]
                .abs()
                .partial_cmp(&matrix[b][col].abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
        if matrix[pivot][col].abs() < 1e-12 {
            return None;
        }
        matrix.swap(col, pivot);
        let divisor = matrix[col][col];
        for item in &mut matrix[col][col..=n] {
            *item /= divisor;
        }
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = matrix[row][col];
            for index in col..=n {
                matrix[row][index] -= factor * matrix[col][index];
            }
        }
    }
    Some(matrix.iter().map(|row| row[n]).collect())
}

fn linear_coeffs(expr: &Expr) -> Option<(BTreeMap<String, f64>, f64)> {
    match expr {
        Expr::Number(value) => Some((BTreeMap::new(), *value)),
        Expr::Variable(name) => {
            let mut coeffs = BTreeMap::new();
            coeffs.insert(name.clone(), 1.0);
            Some((coeffs, 0.0))
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => {
            let (coeffs, constant) = linear_coeffs(expr)?;
            Some((
                coeffs
                    .into_iter()
                    .map(|(name, value)| (name, -value))
                    .collect(),
                -constant,
            ))
        }
        Expr::Binary { op, left, right } => {
            let (left_coeffs, left_constant) = linear_coeffs(left)?;
            let (right_coeffs, right_constant) = linear_coeffs(right)?;
            match op {
                BinaryOp::Add => Some((
                    merge_coeffs(left_coeffs, right_coeffs, 1.0),
                    left_constant + right_constant,
                )),
                BinaryOp::Sub => Some((
                    merge_coeffs(left_coeffs, right_coeffs, -1.0),
                    left_constant - right_constant,
                )),
                BinaryOp::Mul if left_coeffs.is_empty() => Some((
                    scale_coeffs(right_coeffs, left_constant),
                    right_constant * left_constant,
                )),
                BinaryOp::Mul if right_coeffs.is_empty() => Some((
                    scale_coeffs(left_coeffs, right_constant),
                    left_constant * right_constant,
                )),
                BinaryOp::Div if right_coeffs.is_empty() && right_constant.abs() > 1e-12 => Some((
                    scale_coeffs(left_coeffs, 1.0 / right_constant),
                    left_constant / right_constant,
                )),
                _ => None,
            }
        }
        Expr::Call { .. } | Expr::Quantity { .. } => None,
    }
}

fn merge_coeffs(
    mut left: BTreeMap<String, f64>,
    right: BTreeMap<String, f64>,
    right_scale: f64,
) -> BTreeMap<String, f64> {
    for (name, value) in right {
        *left.entry(name).or_insert(0.0) += value * right_scale;
    }
    left
}

fn scale_coeffs(coeffs: BTreeMap<String, f64>, scale: f64) -> BTreeMap<String, f64> {
    coeffs
        .into_iter()
        .map(|(name, value)| (name, value * scale))
        .collect()
}

fn polynomial_coeff(expr: &Expr, variable: &str) -> Option<BTreeMap<usize, f64>> {
    match expr {
        Expr::Number(value) => Some(BTreeMap::from([(0, *value)])),
        Expr::Variable(name) if name == variable => Some(BTreeMap::from([(1, 1.0)])),
        Expr::Variable(_) => None,
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => Some(scale_poly(polynomial_coeff(expr, variable)?, -1.0)),
        Expr::Binary { op, left, right } => match op {
            BinaryOp::Add => Some(add_poly(
                polynomial_coeff(left, variable)?,
                polynomial_coeff(right, variable)?,
                1.0,
            )),
            BinaryOp::Sub => Some(add_poly(
                polynomial_coeff(left, variable)?,
                polynomial_coeff(right, variable)?,
                -1.0,
            )),
            BinaryOp::Mul => multiply_poly(
                polynomial_coeff(left, variable)?,
                polynomial_coeff(right, variable)?,
            ),
            BinaryOp::Div => {
                let denominator = polynomial_coeff(right, variable)?;
                if denominator.len() == 1 {
                    let constant = *denominator.get(&0)?;
                    if constant.abs() > 1e-12 {
                        return Some(scale_poly(
                            polynomial_coeff(left, variable)?,
                            1.0 / constant,
                        ));
                    }
                }
                None
            }
            BinaryOp::Pow => {
                if let (Expr::Variable(name), Expr::Number(power)) = (&**left, &**right)
                    && name == variable
                    && power.fract().abs() < 1e-12
                    && *power >= 0.0
                    && *power <= 2.0
                {
                    return Some(BTreeMap::from([(*power as usize, 1.0)]));
                }
                None
            }
        },
        Expr::Call { .. } | Expr::Quantity { .. } => None,
    }
}

fn add_poly(
    mut left: BTreeMap<usize, f64>,
    right: BTreeMap<usize, f64>,
    right_scale: f64,
) -> BTreeMap<usize, f64> {
    for (degree, value) in right {
        *left.entry(degree).or_insert(0.0) += value * right_scale;
    }
    left.retain(|_, value| value.abs() > 1e-12);
    left
}

fn scale_poly(poly: BTreeMap<usize, f64>, scale: f64) -> BTreeMap<usize, f64> {
    poly.into_iter()
        .map(|(degree, value)| (degree, value * scale))
        .filter(|(_, value)| value.abs() > 1e-12)
        .collect()
}

fn multiply_poly(
    left: BTreeMap<usize, f64>,
    right: BTreeMap<usize, f64>,
) -> Option<BTreeMap<usize, f64>> {
    let mut output = BTreeMap::new();
    for (left_degree, left_value) in left {
        for (right_degree, right_value) in &right {
            let degree = left_degree + right_degree;
            if degree > 2 {
                return None;
            }
            *output.entry(degree).or_insert(0.0) += left_value * right_value;
        }
    }
    output.retain(|_, value| value.abs() > 1e-12);
    Some(output)
}

fn simplify_quantity_binary(op: BinaryOp, left: &Expr, right: &Expr) -> Option<Expr> {
    match (left, right) {
        (
            Expr::Quantity {
                value: left_value,
                unit: left_unit,
            },
            Expr::Quantity {
                value: right_value,
                unit: right_unit,
            },
        ) => {
            let left_number = numeric_value(left_value)?;
            let right_number = numeric_value(right_value)?;
            match op {
                BinaryOp::Add | BinaryOp::Sub => {
                    let converted = convert_unit_value(right_number, right_unit, left_unit)?;
                    let value = if op == BinaryOp::Add {
                        left_number + converted
                    } else {
                        left_number - converted
                    };
                    Some(quantity(value, left_unit))
                }
                BinaryOp::Mul => Some(quantity(
                    left_number * right_number,
                    &format!("{left_unit}*{right_unit}"),
                )),
                BinaryOp::Div => Some(quantity(
                    left_number / right_number,
                    &format!("{left_unit}/{right_unit}"),
                )),
                BinaryOp::Pow => None,
            }
        }
        (Expr::Quantity { value, unit }, Expr::Number(number)) => {
            let value = numeric_value(value)?;
            match op {
                BinaryOp::Mul => Some(quantity(value * number, unit)),
                BinaryOp::Div => Some(quantity(value / number, unit)),
                _ => None,
            }
        }
        (Expr::Number(number), Expr::Quantity { value, unit }) => {
            let value = numeric_value(value)?;
            match op {
                BinaryOp::Mul => Some(quantity(number * value, unit)),
                BinaryOp::Div => Some(quantity(number / value, &format!("1/{unit}"))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn numeric_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Number(value) if value.is_finite() => Some(*value),
        _ => None,
    }
}

fn quantity(value: f64, unit: &str) -> Expr {
    Expr::Quantity {
        value: Box::new(Expr::Number(value)),
        unit: unit.to_string(),
    }
}

fn convert_unit_value(value: f64, from: &str, to: &str) -> Option<f64> {
    let (from_dimension, from_factor) = unit_factor(from)?;
    let (to_dimension, to_factor) = unit_factor(to)?;
    if from_dimension != to_dimension {
        return None;
    }
    Some(value * from_factor / to_factor)
}

fn unit_factor(unit: &str) -> Option<(&'static str, f64)> {
    match unit {
        "mm" => Some(("length", 0.001)),
        "cm" => Some(("length", 0.01)),
        "m" => Some(("length", 1.0)),
        "km" => Some(("length", 1000.0)),
        "ms" => Some(("time", 0.001)),
        "s" => Some(("time", 1.0)),
        "min" => Some(("time", 60.0)),
        "h" => Some(("time", 3600.0)),
        "g" => Some(("mass", 0.001)),
        "kg" => Some(("mass", 1.0)),
        _ => None,
    }
}

fn contains_non_finite(expr: &Expr) -> bool {
    match expr {
        Expr::Number(value) => !value.is_finite(),
        Expr::Unary { expr, .. } => contains_non_finite(expr),
        Expr::Binary { left, right, .. } => contains_non_finite(left) || contains_non_finite(right),
        Expr::Call { args, .. } => args.iter().any(contains_non_finite),
        Expr::Quantity { value, .. } => contains_non_finite(value),
        Expr::Variable(_) => false,
    }
}

fn add(left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn sub(left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn mul(left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn div(left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op: BinaryOp::Div,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn pow(left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op: BinaryOp::Pow,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        name: name.to_string(),
        args,
    }
}

fn expr_value(expr: &Expr, precision: usize) -> String {
    match expr {
        Expr::Number(value) => format_precision(*value, precision),
        Expr::Quantity { value, unit } => format!("{} {unit}", expr_value(value, precision)),
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

    #[test]
    fn computes_calculus_mvp_cases() {
        let doc = parse_document(
            "diff(sin(x^2), x)\nintegrate(x^2, x)\nlim(x^2 + 1, x, 3)",
            SourceFormat::Math,
        )
        .unwrap();
        let output = solve_document(&doc, &SolveOptions::default());
        assert_eq!(
            output.results[0].value.as_deref(),
            Some("(cos((x ^ 2)) * (2 * x))")
        );
        assert_eq!(output.results[1].value.as_deref(), Some("((x ^ 3) / 3)"));
        assert_eq!(output.results[2].value.as_deref(), Some("10"));
    }

    #[test]
    fn solves_quadratic_and_linear_system() {
        let doc = parse_document("x^2 + 5*x + 6 = 0\nx + y = 5\nx - y = 1", SourceFormat::Math)
            .unwrap();
        let output = solve_document(&doc, &SolveOptions::default());
        assert_eq!(output.results[0].value.as_deref(), Some("x1 = -2, x2 = -3"));
        assert_eq!(
            output.results.last().and_then(|result| result.value.as_deref()),
            Some("x = 3, y = 2")
        );
    }

    #[test]
    fn solves_for_specific_variable() {
        let doc = parse_document("x + y = 5", SourceFormat::Math).unwrap();
        let output = solve_document(
            &doc,
            &SolveOptions {
                solve_for: Some("x".to_string()),
                ..SolveOptions::default()
            },
        );
        assert_eq!(output.results[0].value.as_deref(), Some("x = (5 - y)"));
    }

    #[test]
    fn evaluates_units_and_records_assumptions() {
        let doc = parse_document(
            "assume x > 0\ndistance=12 km\ntime=30 min\nspeed=distance/time\nheight=1500 m\ntotal=distance+height",
            SourceFormat::Math,
        )
        .unwrap();
        let output = solve_document(&doc, &SolveOptions::default());
        assert_eq!(output.assumptions[0].condition, "> 0");
        assert_eq!(output.results[3].value.as_deref(), Some("0.4 km/min"));
        assert_eq!(output.results[5].value.as_deref(), Some("13.5 km"));
    }
}
