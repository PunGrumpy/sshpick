use std::path::Path;

pub(crate) fn env_label(path: &Path, first_line: &str) -> String {
    from_comment(first_line).map_or_else(|| from_stem(path), str::to_owned)
}

fn from_comment(line: &str) -> Option<&str> {
    let label = line.trim().strip_prefix('#')?.trim_start_matches(' ');
    let label = before_first(label, &[" — ", " - "]);
    let label = before_first(label, &[" ("]).trim_end();
    (!label.is_empty()).then_some(label)
}

fn before_first<'a>(text: &'a str, separators: &[&str]) -> &'a str {
    let end = separators
        .iter()
        .filter_map(|separator| text.find(separator))
        .min()
        .unwrap_or(text.len());
    &text[..end]
}

fn from_stem(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let after_digits = stem.trim_start_matches(|c: char| c.is_ascii_digit());
    match after_digits.strip_prefix(['-', '_']) {
        Some(name) if after_digits.len() < stem.len() && !name.is_empty() => name.to_owned(),
        _ => stem.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: [(&str, &str); 11] = [
        (
            "# LAB — personal DevStack / OpenSDN VMs (direct, no bastion)",
            "LAB",
        ),
        (
            "# NEW STAGING (STG-ORG, built Oct 2026; becomes plain \"staging\" once the migration is done)",
            "NEW STAGING",
        ),
        (
            "# OLD STAGING — stg.thaiopenstack.com (kolla, region NCP-TH).",
            "OLD STAGING",
        ),
        ("#LAB-tight - rest", "LAB-tight"),
        ("##  Double hash - x", "#  Double hash"),
        ("# PROD", "PROD"),
        ("# A - B - C", "A"),
        ("# Name (x) - y", "Name"),
        ("# LAB-ONE (x)", "LAB-ONE"),
        ("# (only paren)", "(only paren)"),
        ("# Has trailing space  — x", "Has trailing space"),
    ];

    #[test]
    fn comment_labels_match_the_prototype_sed() {
        for (line, want) in CASES {
            assert_eq!(env_label(Path::new("10-x.conf"), line), want, "{line:?}");
        }
    }

    #[test]
    fn stem_label_when_line_one_is_not_a_label() {
        assert_eq!(env_label(Path::new("30-prod.conf"), "Host a"), "prod");
        assert_eq!(env_label(Path::new("10_lab.conf"), "#"), "lab");
        assert_eq!(env_label(Path::new("prod.conf"), ""), "prod");
        assert_eq!(env_label(Path::new("42.conf"), "Host a"), "42");
    }
}
