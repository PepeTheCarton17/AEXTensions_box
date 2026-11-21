pub fn parse_repo_target(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (None, String::new());
    }

    let without_git = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let without_scheme = without_git
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("ssh://")
        .trim_start_matches("git@")
        .trim_start_matches("www.");
    let without_host = without_scheme
        .trim_start_matches("github.com/")
        .trim_start_matches("github.com:")
        .trim_end_matches('/');

    let parts: Vec<&str> = without_host
        .split('/')
        .filter(|p| !p.trim().is_empty())
        .collect();

    match parts.as_slice() {
        [] => (None, String::new()),
        [repo] => (None, (*repo).to_string()),
        [owner, repo, ..] => (Some((*owner).to_string()), (*repo).to_string()),
    }
}

pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let (owner, repo) = parse_repo_target(url);
    let owner = owner?;
    if repo.is_empty() {
        None
    } else {
        Some((owner, repo))
    }
}

pub(crate) fn sanitize_repo_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => out.push(ch),
            _ => out.push('-'),
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        "nekoyomi-extensions".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::parse_repo_target;

    #[test]
    fn parses_repo_targets() {
        assert_eq!(parse_repo_target(""), (None, String::new()));
        assert_eq!(
            parse_repo_target("unbox_extensions"),
            (None, "unbox_extensions".to_string())
        );
        assert_eq!(
            parse_repo_target("PepeTheCarton17/unbox_extensions"),
            (
                Some("PepeTheCarton17".to_string()),
                "unbox_extensions".to_string()
            )
        );
        assert_eq!(
            parse_repo_target("https://github.com/PepeTheCarton17/unbox_extensions.git"),
            (
                Some("PepeTheCarton17".to_string()),
                "unbox_extensions".to_string()
            )
        );
    }
}
