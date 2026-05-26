use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::ast::{MathDocument, Statement};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VariableInfo {
    pub name: String,
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DependencyGraph {
    pub variables: Vec<VariableInfo>,
    pub cycles: Vec<String>,
}

impl DependencyGraph {
    pub fn mermaid(&self) -> String {
        let mut lines = vec!["graph TD".to_string()];
        for variable in &self.variables {
            if variable.depends_on.is_empty() {
                lines.push(format!("  {}", graph_id(&variable.name)));
            } else {
                for dep in &variable.depends_on {
                    lines.push(format!(
                        "  {} --> {}",
                        graph_id(&variable.name),
                        graph_id(dep)
                    ));
                }
            }
        }
        lines.join("\n")
    }

    pub fn dot(&self) -> String {
        let mut lines = vec!["digraph tinfo_math {".to_string()];
        for variable in &self.variables {
            if variable.depends_on.is_empty() {
                lines.push(format!("  \"{}\";", escape_dot(&variable.name)));
            } else {
                for dep in &variable.depends_on {
                    lines.push(format!(
                        "  \"{}\" -> \"{}\";",
                        escape_dot(&variable.name),
                        escape_dot(dep)
                    ));
                }
            }
        }
        lines.push("}".to_string());
        lines.join("\n")
    }
}

pub fn build_dependency_graph(document: &MathDocument) -> DependencyGraph {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut expression_index = 1;
    let mut equation_index = 1;

    for statement in &document.statements {
        match statement {
            Statement::Assignment { name, expr } => {
                let mut deps = BTreeSet::new();
                expr.collect_vars(&mut deps);
                deps.remove(name);
                map.insert(name.clone(), deps);
            }
            Statement::Equation { left, right } => {
                let mut deps = BTreeSet::new();
                left.collect_vars(&mut deps);
                right.collect_vars(&mut deps);
                map.insert(format!("equation_{equation_index}"), deps);
                equation_index += 1;
            }
            Statement::Assumption { variable, .. } => {
                map.entry(variable.clone()).or_default();
            }
            Statement::Expression(expr) => {
                let mut deps = BTreeSet::new();
                expr.collect_vars(&mut deps);
                map.insert(format!("expression_{expression_index}"), deps);
                expression_index += 1;
            }
        }
    }

    let variables = map
        .iter()
        .map(|(name, deps)| VariableInfo {
            name: name.clone(),
            depends_on: deps.iter().cloned().collect(),
        })
        .collect::<Vec<_>>();
    let cycles = detect_cycles(&map);
    DependencyGraph { variables, cycles }
}

fn detect_cycles(map: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    let mut cycles = Vec::new();
    for name in map.keys() {
        let mut path = Vec::new();
        if visits(name, name, map, &mut path) {
            cycles.push(path.join(" -> "));
        }
    }
    cycles.sort();
    cycles.dedup();
    cycles
}

fn visits(
    target: &str,
    current: &str,
    map: &BTreeMap<String, BTreeSet<String>>,
    path: &mut Vec<String>,
) -> bool {
    path.push(current.to_string());
    if let Some(deps) = map.get(current) {
        for dep in deps {
            if dep == target {
                path.push(dep.clone());
                return true;
            }
            if map.contains_key(dep)
                && !path.iter().any(|item| item == dep)
                && visits(target, dep, map, path)
            {
                return true;
            }
        }
    }
    path.pop();
    false
}

fn graph_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::ast::SourceFormat;
    use crate::math::parser::parse_document;

    #[test]
    fn graph_tracks_dependencies() {
        let doc = parse_document("a=5\nb=a^2", SourceFormat::Math).unwrap();
        let graph = build_dependency_graph(&doc);
        assert!(graph.mermaid().contains("b --> a"));
    }
}
