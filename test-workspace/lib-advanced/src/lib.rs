use lib_math::{add, multiply};
use lib_parser::parse_int;

pub fn eval_expression(expr: &str) -> Result<i32, String> {
    // Simple expression evaluator: "5 + 3" or "4 * 2"
    let parts: Vec<&str> = expr.split_whitespace().collect();
    
    if parts.len() != 3 {
        return Err("Invalid expression format".to_string());
    }
    
    let a = parse_int(parts[0])?;
    let b = parse_int(parts[2])?;
    
    match parts[1] {
        "+" => Ok(add(a, b)),
        "*" => Ok(multiply(a, b)),
        _ => Err("Unsupported operator".to_string()),
    }
}

pub fn batch_eval(expressions: &[&str]) -> Vec<Result<i32, String>> {
    expressions.iter().map(|e| eval_expression(e)).collect()
}

