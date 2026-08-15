use crate::collectors::traits::Platform;

pub fn build_commit_url(
    platform: Platform,
    upstream_url: &str,
    owner: &str,
    repo: &str,
    sha: &str,
    branch: &str,
) -> String {
    let fallback_url = match platform {
        Platform::Gitee => format!("https://gitee.com/{owner}/{repo}/commit/{sha}"),
        Platform::AtomGit => format!("https://atomgit.com/{owner}/{repo}/commits/detail/{sha}"),
        _ => String::new(),
    };
    let url = upstream_url.trim();
    let url = if url.is_empty() {
        fallback_url.as_str()
    } else {
        url
    };

    normalize_commit_url_for_branch(platform, url, branch)
}

pub fn normalize_commit_url_for_branch(platform: Platform, url: &str, branch: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }

    match platform {
        Platform::AtomGit => append_query_param_if_missing(url, "ref", branch),
        _ => url.to_string(),
    }
}

fn append_query_param_if_missing(url: &str, key: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || query_contains_key(url, key) {
        return url.to_string();
    }

    let encoded = urlencoding::encode(value);
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}{key}={encoded}")
}

fn query_contains_key(url: &str, key: &str) -> bool {
    let Some((_, query)) = url.split_once('?') else {
        return false;
    };
    query
        .split('&')
        .filter_map(|part| part.split_once('=').map(|(name, _)| name).or(Some(part)))
        .any(|name| name == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomgit_url_gets_ref_branch() {
        let url = build_commit_url(
            Platform::AtomGit,
            "https://atomgit.com/src-openeuler/bash/commits/detail/sha",
            "src-openeuler",
            "bash",
            "sha",
            "openEuler-20.03-LTS-SP4",
        );

        assert_eq!(
            url,
            "https://atomgit.com/src-openeuler/bash/commits/detail/sha?ref=openEuler-20.03-LTS-SP4"
        );
    }

    #[test]
    fn atomgit_url_does_not_duplicate_ref() {
        let url = normalize_commit_url_for_branch(
            Platform::AtomGit,
            "https://atomgit.com/src-openeuler/bash/commits/detail/sha?ref=openEuler-20.03-LTS-SP4",
            "openEuler-24.03-LTS-SP1",
        );

        assert_eq!(
            url,
            "https://atomgit.com/src-openeuler/bash/commits/detail/sha?ref=openEuler-20.03-LTS-SP4"
        );
    }

    #[test]
    fn atomgit_fallback_uses_detail_url_with_ref() {
        let url = build_commit_url(
            Platform::AtomGit,
            "",
            "src-openeuler",
            "bash",
            "branch-sha",
            "openEuler-24.03-LTS-SP1",
        );

        assert_eq!(
            url,
            "https://atomgit.com/src-openeuler/bash/commits/detail/branch-sha?ref=openEuler-24.03-LTS-SP1"
        );
    }

    #[test]
    fn gitee_url_is_unchanged() {
        let url = build_commit_url(
            Platform::Gitee,
            "",
            "src-openeuler",
            "bash",
            "sha",
            "openEuler-20.03-LTS-SP4",
        );

        assert_eq!(url, "https://gitee.com/src-openeuler/bash/commit/sha");
    }
}
