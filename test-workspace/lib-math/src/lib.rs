use lib_common::Config;

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn factorial(n: u32) -> u64 {
    (1..=n as u64).product()
}

pub fn calculate_with_config(a: i32, b: i32, config: &Config) -> i32 {
    let result = add(a, b);
    if config.verbose {
        println!("Result: {}", result);
    }
    result
}

