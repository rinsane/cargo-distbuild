use lib_common::{get_app_name, get_version, Config};
use lib_math::{add, factorial};
use lib_utils::format_message;
use lib_parser::count_tokens;
use lib_advanced::eval_expression;

fn main() {
    println!("Welcome to {} v{}", get_app_name(), get_version());
    
    let config = Config {
        debug: true,
        verbose: true,
    };
    
    // Test math
    let sum = add(5, 3);
    println!("{}", format_message(&format!("5 + 3 = {}", sum)));
    
    let fact = factorial(5);
    println!("{}", format_message(&format!("5! = {}", fact)));
    
    // Test parser
    let text = "Hello world from distributed build";
    let token_count = count_tokens(text);
    println!("{}", format_message(&format!("'{}' has {} tokens", text, token_count)));
    
    // Test advanced
    match eval_expression("10 + 20") {
        Ok(result) => println!("{}", format_message(&format!("10 + 20 = {}", result))),
        Err(e) => eprintln!("Error: {}", e),
    }
    
    if config.debug {
        println!("Debug mode enabled!");
    }
}

