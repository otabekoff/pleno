//! "Did you mean ...?" suggestions for error messages: the known name closest
//! to a misspelled one. Only computed while an error is being reported.

/// The optimal string alignment distance between `a` and `b`: the number of
/// single-character insertions, deletions, substitutions and swaps of two
/// neighbouring characters that turn one into the other.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return a.len().max(b.len());
    }
    // Three rows: the one before the previous one is needed for swaps.
    let mut before: Vec<usize> = vec![0; b.len() + 1];
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current: Vec<usize> = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        current[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(before[j - 2] + 1);
            }
            current[j] = best;
        }
        std::mem::swap(&mut before, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// The candidate closest to `name`, if one is close enough and no other,
/// different candidate is as close.
///
/// Close enough means at most `max(1, len / 3)` edits apart, ignoring case
/// (a difference in case alone always counts as close) and fewer edits
/// than either name has characters, so `b` never suggests `a`; among equally close
/// candidates the one that also matches the case wins. With nothing that
/// close, a name of four or more characters suggests the one candidate it
/// starts, as `contains` does `containsKey`. `name` itself is never
/// suggested.
pub fn best_match<'a, I>(name: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let lower = name.to_lowercase();
    let length = lower.chars().count();
    let threshold = (length / 3).max(1);
    let mut best: Option<((usize, usize), &'a str)> = None;
    let mut tied = false;
    let mut prefixed: Option<&'a str> = None;
    let mut prefix_tied = false;
    for candidate in candidates {
        if candidate == name || candidate.is_empty() {
            continue;
        }
        let candidate_lower = candidate.to_lowercase();
        let candidate_length = candidate_lower.chars().count();
        if length >= 4 && candidate_lower.starts_with(&lower) {
            match prefixed {
                Some(other) if other != candidate => prefix_tied = true,
                Some(_) => {}
                None => prefixed = Some(candidate),
            }
        }
        if candidate_length.abs_diff(length) > threshold {
            continue;
        }
        let folded = edit_distance(&lower, &candidate_lower);
        // Replacing every character of a short name is no spelling mistake.
        if folded > threshold || folded >= length.min(candidate_length) {
            continue;
        }
        let key = (folded, edit_distance(name, candidate));
        match best {
            Some((best_key, best_name)) if key == best_key => {
                if best_name != candidate {
                    tied = true;
                }
            }
            Some((best_key, _)) if key > best_key => {}
            _ => {
                best = Some((key, candidate));
                tied = false;
            }
        }
    }
    match best {
        Some((_, found)) if !tied => Some(found),
        Some(_) => None,
        None if !prefix_tied => prefixed,
        None => None,
    }
}

/// `"; did you mean 'x'?"` for the candidate [`best_match`] picks, or an
/// empty string: ready to append to an error message.
pub fn did_you_mean<'a, I>(name: &str, candidates: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    match best_match(name, candidates) {
        Some(found) => format!("; did you mean '{found}'?"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_edits() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("", "ab"), 2);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("push", "pushh"), 1);
        assert_eq!(edit_distance("flor", "floor"), 1);
        assert_eq!(edit_distance("same", "same"), 0);
    }

    #[test]
    fn distance_counts_a_swap_as_one_edit() {
        assert_eq!(edit_distance("lenght", "length"), 1);
        assert_eq!(edit_distance("nmae", "name"), 1);
        assert_eq!(edit_distance("ab", "ba"), 1);
    }

    #[test]
    fn distance_works_on_characters() {
        assert_eq!(edit_distance("héllo", "hello"), 1);
        assert_eq!(edit_distance("日本", "日本語"), 1);
    }

    #[test]
    fn suggests_the_closest_candidate() {
        let names = ["length", "push", "pop", "filter"];
        assert_eq!(best_match("lenght", names), Some("length"));
        assert_eq!(best_match("pushh", names), Some("push"));
        assert_eq!(best_match("fliter", names), Some("filter"));
    }

    #[test]
    fn suggests_nothing_when_too_far() {
        let names = ["length", "push"];
        assert_eq!(best_match("xyz", names), None);
        // Two edits on a five-letter name is over the threshold of one.
        assert_eq!(best_match("pzshx", names), None);
        assert_eq!(best_match("anything", Vec::<&str>::new()), None);
    }

    #[test]
    fn short_names_need_a_character_in_common() {
        assert_eq!(best_match("b", ["a"]), None);
        assert_eq!(best_match("y", ["x", "Y"]), Some("Y"));
        assert_eq!(best_match("ab", ["cd"]), None);
        assert_eq!(best_match("ab", ["ac"]), Some("ac"));
    }

    #[test]
    fn never_suggests_the_name_itself() {
        assert_eq!(best_match("push", ["push"]), None);
        assert_eq!(best_match("push", ["push", "posh"]), Some("posh"));
    }

    #[test]
    fn suggests_nothing_when_two_are_equally_close() {
        assert_eq!(best_match("cat", ["bat", "hat"]), None);
        // The same name twice is not a tie.
        assert_eq!(best_match("cat", ["bat", "bat"]), Some("bat"));
    }

    #[test]
    fn case_differences_count_as_close() {
        assert_eq!(
            best_match("touppercase", ["toUpperCase"]),
            Some("toUpperCase")
        );
        assert_eq!(best_match("math", ["Math", "path"]), Some("Math"));
        // Equally close ignoring case: the one with the same case wins.
        assert_eq!(best_match("Pat", ["Path", "path"]), Some("Path"));
    }

    #[test]
    fn a_long_enough_prefix_suggests_the_longer_name() {
        assert_eq!(
            best_match("contains", ["containsKey", "keys"]),
            Some("containsKey")
        );
        assert_eq!(best_match("map", ["mapValues"]), None);
        assert_eq!(best_match("user", ["userName", "userId"]), None);
        // A close spelling beats a prefix.
        assert_eq!(best_match("user", ["users", "userName"]), Some("users"));
    }

    #[test]
    fn did_you_mean_formats_the_suffix() {
        assert_eq!(
            did_you_mean("lenght", ["length"]),
            "; did you mean 'length'?"
        );
        assert_eq!(did_you_mean("zzz", ["length"]), "");
    }
}
