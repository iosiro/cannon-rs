/// Utility function to convert an identifier to the desired case.
/// If the identifier has consecutive uppercase characters, it will remain unchanged (like USDToken),
/// otherwise, it converts to camelCase (like MyToken -> myToken).
pub fn to_lower_camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut prev_is_uppercase = false;
    let mut consecutive_uppercase = true;

    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            // Check if we are in the first few characters or all are uppercase so far
            if i > 0 && !prev_is_uppercase {
                consecutive_uppercase = false;
            }
            prev_is_uppercase = true;
        } else {
            prev_is_uppercase = false;
        }

        result.push(c);
    }

    // Convert to camelCase if not entirely uppercase sequence at the start
    if !consecutive_uppercase {
        result[..1].to_lowercase().to_string() + &result[1..]
    } else {
        result // Return as-is if consecutive uppercase
    }
}

/// Utility function to convert an identifier to constant case.
/// Keeps consecutive uppercase letters (like "USD") intact and ensures proper formatting.
pub fn to_constant_case(name: &str) -> String {
    let mut result = String::new();
    let mut prev_is_uppercase = false;

    for c in name.chars() {
        if c.is_uppercase() {
            if !prev_is_uppercase {
                result.push('_');
            }
            prev_is_uppercase = true;
        } else {
            prev_is_uppercase = false;
        }

        result.push(c);
    }

    result.to_uppercase()
}


pub fn repeat_string(s: &str, count: usize) -> String {
    (0..count).map(|_| s).collect()
}
