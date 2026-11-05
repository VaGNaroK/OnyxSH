//! OSC 7 Working Directory Parser (Port of `src/onyxsh/utils/osc7.py`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Osc7Info {
    pub hostname: String,
    pub path: String,
    pub display_path: String,
}

/// Formats an absolute path into a user-friendly display path (`~/...` or `.../a/b/c`).
pub fn create_display_path(path: &str, home_path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    if !home_path.is_empty() && path.starts_with(home_path) {
        if path == home_path {
            return path.to_string();
        }
        if path.as_bytes().get(home_path.len()) == Some(&b'/') {
            return format!("~{}", &path[home_path.len()..]);
        }
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 4 {
        let tail = &parts[parts.len() - 3..];
        return format!(".../{}", tail.join("/"));
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_path_formatting() {
        let home = "/home/vagnarok";
        assert_eq!(create_display_path("/", home), "/");
        assert_eq!(create_display_path("/home/vagnarok", home), "/home/vagnarok");
        assert_eq!(
            create_display_path("/home/vagnarok/OnyxSH/src", home),
            "~/OnyxSH/src"
        );
        assert_eq!(
            create_display_path("/var/lib/docker/volumes/app/data", home),
            ".../volumes/app/data"
        );
    }
}
