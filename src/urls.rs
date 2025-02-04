use std::process::Command;
use url::Url;

/// Checks if a given string is a valid URL or domain
pub fn is_valid_url(url_str: &str) -> bool {
    // First try parsing as a complete URL
    if let Ok(url) = Url::parse(url_str) {
        return url.scheme() == "http" || url.scheme() == "https";
    }

    // Check if it starts with www
    if url_str.starts_with("www.") {
        return is_valid_domain(&url_str[4..]);
    }

    // Check if it's a valid domain
    is_valid_domain(url_str)
}

/// Helper function to validate domain-like strings
fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() {
        return false;
    }

    // Basic domain validation
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // Check each part of the domain
    for part in parts {
        if part.is_empty() {
            return false;
        }
        // Check if part contains only valid characters
        if !part.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return false;
        }
    }

    true
}

/// Normalizes a URL by ensuring it has https:// prefix
fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.starts_with("www.") {
        format!("https://{}", url)
    } else {
        format!("https://{}", url)
    }
}

/// Opens the given URL in the default system browser
/// Returns Ok(()) if successful, or an error if the URL is invalid or couldn't be opened
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_valid_url(url) {
        return Err("Invalid URL".to_string());
    }

    let normalized_url = normalize_url(url);

    #[cfg(target_os = "linux")]
    {
        // Try different commands in order
        let browsers = [
            ("xdg-open", vec![&normalized_url]),
            ("sensible-browser", vec![&normalized_url]),
            ("x-www-browser", vec![&normalized_url]),
            ("gnome-open", vec![&normalized_url]),
        ];

        for (cmd, args) in browsers.iter() {
            if let Ok(status) = Command::new(cmd).args(args).status() {
                if status.success() {
                    return Ok(());
                }
            }
        }
        return Err("Failed to open URL with any available browser".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(status) = Command::new("open").arg(&normalized_url).status() {
            if status.success() {
                return Ok(());
            }
        }
        return Err("Failed to open URL".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(status) = Command::new("cmd")
            .args(["/C", "start", &normalized_url])
            .status()
        {
            if status.success() {
                return Ok(());
            }
        }
        return Err("Failed to open URL".to_string());
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return Err("Unsupported operating system".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_urls() {
        // Test complete URLs
        assert!(is_valid_url("https://www.rust-lang.org"));
        assert!(is_valid_url("http://example.com"));

        // Test www formats
        assert!(is_valid_url("www.example.com"));
        assert!(is_valid_url("www.rust-lang.org"));

        // Test bare domains
        assert!(is_valid_url("example.com"));
        assert!(is_valid_url("rust-lang.org"));

        // Test invalid URLs
        assert!(!is_valid_url("not-a-url"));
        assert!(!is_valid_url("http://"));
        assert!(!is_valid_url("www."));
        assert!(!is_valid_url(".com"));
        assert!(!is_valid_url("example"));
    }

    #[test]
    fn test_url_normalization() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
        assert_eq!(normalize_url("www.example.com"), "https://www.example.com");
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(normalize_url("http://example.com"), "http://example.com");
    }
}
