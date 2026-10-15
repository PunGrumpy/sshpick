const MIN_LENGTH: usize = 3;

/// The one candidate that `input` is a single edit away from, or `None`. A candidate is a
/// `(word, answer)` pair: aliases share an answer, so `lst` suggests `list` even though `ls` is
/// one edit away too. Leading dashes are ignored on both sides, a word that is already a
/// candidate is never corrected, and nothing under three characters is guessed at. There is no
/// similarity score to fall back on: staying silent beats `bogus` becoming `logs`.
pub(crate) fn closest<'a>(input: &str, candidates: &[(&str, &'a str)]) -> Option<&'a str> {
    let input: Vec<char> = input.trim_start_matches('-').chars().collect();
    if input.len() < MIN_LENGTH {
        return None;
    }
    let mut found = None;
    for (word, answer) in candidates {
        let word: Vec<char> = word.trim_start_matches('-').chars().collect();
        if word == input {
            return None;
        }
        if one_edit_apart(&input, &word) {
            match found {
                Some(other) if other != *answer => return None,
                _ => found = Some(*answer),
            }
        }
    }
    found
}

/// One substitution, one insertion, one deletion or one swap of neighbours.
fn one_edit_apart(a: &[char], b: &[char]) -> bool {
    match a.len().abs_diff(b.len()) {
        0 => {
            let differing: Vec<usize> = (0..a.len()).filter(|&at| a[at] != b[at]).collect();
            match differing[..] {
                [_] => true,
                [first, second] => {
                    second == first + 1 && a[first] == b[second] && a[second] == b[first]
                }
                _ => false,
            }
        }
        1 => {
            let (long, short) = if a.len() > b.len() { (a, b) } else { (b, a) };
            let shared = long.iter().zip(short).take_while(|(x, y)| x == y).count();
            long[shared + 1..] == short[shared..]
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    const COMMANDS: &[(&str, &str)] = &[
        ("pick", "pick"),
        ("list", "list"),
        ("ls", "list"),
        ("inspect", "inspect"),
        ("help", "help"),
    ];

    #[test_case("lst" => Some("list"); "a deletion that two aliases agree on")]
    #[test_case("lsit" => Some("list"); "a swap of neighbours")]
    #[test_case("inspekt" => Some("inspect"); "a substitution")]
    #[test_case("inspec" => Some("inspect"); "a missing last letter")]
    #[test_case("hhelp" => Some("help"); "an extra letter")]
    #[test_case("pik" => Some("pick"); "three letters is enough")]
    #[test_case("bogus" => None; "nothing is close")]
    #[test_case("ls" => None; "a candidate is never corrected")]
    #[test_case("list" => None; "a canonical name is never corrected")]
    #[test_case("lt" => None; "under three characters")]
    #[test_case("" => None; "nothing at all")]
    #[test_case("lstt" => None; "two edits")]
    #[test_case("sli" => None; "a rotation is two edits")]
    fn commands(input: &str) -> Option<&'static str> {
        closest(input, COMMANDS)
    }

    #[test]
    fn two_different_answers_make_no_suggestion() {
        let candidates = [("cat", "cat"), ("cot", "cot")];
        assert_eq!(closest("cut", &candidates), None);
        assert_eq!(closest("cats", &candidates), Some("cat"));
    }

    #[test]
    fn leading_dashes_are_ignored_and_the_answer_keeps_its_own_spelling() {
        let options = [
            ("--list", "--list"),
            ("--json", "--json"),
            ("--help", "--help"),
        ];
        assert_eq!(closest("--lst", &options), Some("--list"));
        assert_eq!(closest("--josn", &options), Some("--json"));
        assert_eq!(closest("-lst", &options), Some("--list"));
        assert_eq!(closest("--list", &options), None);
        assert_eq!(closest("--bogus", &options), None);
    }

    #[test]
    fn multibyte_words_are_compared_by_character() {
        let candidates = [("café", "café")];
        assert_eq!(closest("cafe", &candidates), Some("café"));
    }
}
