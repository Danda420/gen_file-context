pub fn escape_regex(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    
    for c in s.chars() {
        if matches!(c, '[' | ']' | '.' | '^' | '$' | '*' | '+' | '?' | '{' | '}' | '|' | '(' | ')') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}