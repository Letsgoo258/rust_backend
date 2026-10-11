/// Normalizes raw phone numbers into clean +91XXXXXXXXXX format without spaces or formatting characters.
pub fn normalize_phone_number(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Filter out all non-digit and non-plus characters (remove spaces, dashes, parentheses)
    let cleaned: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();

    if cleaned.starts_with("+91") {
        cleaned
    } else if cleaned.starts_with("91") && cleaned.len() == 12 {
        format!("+{}", cleaned)
    } else if cleaned.starts_with('0') && cleaned.len() == 11 {
        format!("+91{}", &cleaned[1..])
    } else if cleaned.len() == 10 && cleaned.chars().all(|c| c.is_ascii_digit()) {
        format!("+91{}", cleaned)
    } else if cleaned.starts_with('+') {
        cleaned
    } else {
        format!("+91{}", cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phone_normalization() {
        assert_eq!(normalize_phone_number("9876543210"), "+919876543210");
        assert_eq!(normalize_phone_number("+91 98765 43210"), "+919876543210");
        assert_eq!(normalize_phone_number("09876543210"), "+919876543210");
        assert_eq!(normalize_phone_number("919876543210"), "+919876543210");
        assert_eq!(normalize_phone_number("+91-98765-43210"), "+919876543210");
    }
}
