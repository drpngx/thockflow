
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditOp {
    Match,      // Characters match
    Substitute, // Wrong character typed
    Insert,     // Extra character in user input
}

// Optimal alignment using DP (Levenshtein distance with path reconstruction)
// O(N*M) where N is input length and M is quote length.
// Since max quote length is ~500, this is fast enough (~250k ops).
pub fn align_incremental(quote: &str, input: &str) -> Vec<(EditOp, Option<char>, Option<char>)> {
    let quote_chars: Vec<char> = quote.chars().collect();
    let input_chars: Vec<char> = input.chars().collect();
    let n = input_chars.len();
    let m = quote_chars.len();

    // dp[i][j] = min cost to align input[0..i] with quote[0..j]
    let mut dp = vec![vec![0u32; m + 1]; n + 1];

    // Initialize first row: input is empty.
    // Cost is j (skipping j quote characters).
    for j in 0..=m {
        dp[0][j] = j as u32;
    }
    // Initialize first column: quote is empty.
    // Cost is i * 2 (inserting i input characters).
    for i in 0..=n {
        dp[i][0] = (i as u32) * 2;
    }

    // Fill DP table
    for i in 1..=n {
        for j in 1..=m {
            let char_match = input_chars[i - 1] == quote_chars[j - 1];
            // Higher substitution cost (2) makes the algorithm prefer 
            // Skipping+Inserting (cost 1+1=2) or just Skipping over 
            // a long chain of substitutions for mismatched words.
            let cost_match = if char_match { 0 } else { 2 };

            let diag = dp[i - 1][j - 1] + cost_match; // Match or Substitute
            let left = dp[i][j - 1] + 1;              // Skip (delete from quote)
            let up = dp[i - 1][j] + 2;                // Insert (add to input) - Cost 2

            dp[i][j] = diag.min(left).min(up);
        }
    }

    // Find the best endpoint in the last row (after consuming all input).
    // We want to minimize cost. In case of ties, we prefer larger j (more quote consumed),
    // as users typically type forward.
    let mut best_j = 0;
    let mut min_cost = u32::MAX;
    for j in 0..=m {
        if dp[n][j] <= min_cost {
            min_cost = dp[n][j];
            best_j = j;
        }
    }

    // Backtrack from (n, best_j) to (0, 0) to reconstruct the path
    let mut result = Vec::new();
    let mut i = n;
    let mut j = best_j;

    while i > 0 || j > 0 {
        let current_cost = dp[i][j];
        
        let char_match = if i > 0 && j > 0 { input_chars[i - 1] == quote_chars[j - 1] } else { false };
        let cost_diag = if char_match { 0 } else { 2 };

        let from_diag = i > 0 && j > 0 && dp[i - 1][j - 1] + cost_diag == current_cost;
        let from_up = i > 0 && dp[i - 1][j] + 2 == current_cost;
        let from_left = j > 0 && dp[i][j - 1] + 1 == current_cost;

        if from_diag {
            // Match or Substitute
            let op = if char_match { EditOp::Match } else { EditOp::Substitute };
            result.push((op, Some(quote_chars[j - 1]), Some(input_chars[i - 1])));
            i -= 1;
            j -= 1;
        } else if from_up {
            // Insertion (extra char in input)
            result.push((EditOp::Insert, None, Some(input_chars[i - 1])));
            i -= 1;
        } else if from_left {
            // Skip (missed char in quote)
            result.push((EditOp::Substitute, Some(quote_chars[j - 1]), None));
            j -= 1;
        } else {
            // Should be unreachable if logic is correct
            break;
        }
    }

    result.reverse();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_match() {
        let quote = "hello world";
        let input = "hello world";
        let res = align_incremental(quote, input);
        assert_eq!(res.len(), 11);
        for (op, _, _) in res {
            assert_eq!(op, EditOp::Match);
        }
    }

    #[test]
    fn test_single_typo() {
        let quote = "hello world";
        let input = "hallo world"; // 'e' -> 'a'
        let res = align_incremental(quote, input);
        
        assert_eq!(res[1].0, EditOp::Substitute); // e -> a
        assert_eq!(res[0].0, EditOp::Match);
        assert_eq!(res[2].0, EditOp::Match);
    }

    #[test]
    fn test_insertion() {
        let quote = "hello";
        let input = "heello"; // extra 'e'
        let res = align_incremental(quote, input);
        
        // h, e, insert e, l, l, o
        // or h, insert e, e, l, l, o
        // Just checking generally we have an insert
        let inserts = res.iter().filter(|(op, _, _)| *op == EditOp::Insert).count();
        assert_eq!(inserts, 1);
    }

    #[test]
    fn test_skip_char() {
        let quote = "hello";
        let input = "hllo"; // missed 'e'
        let res = align_incremental(quote, input);
        
        // h, skip e, l, l, o
        let skips = res.iter().filter(|(op, _, input_char)| *op == EditOp::Substitute && input_char.is_none()).count();
        assert_eq!(skips, 1);
    }

    #[test]
    fn test_skip_word() {
        let quote = "the quick brown fox";
        let input = "the brown fox"; // missed "quick "
        let res = align_incremental(quote, input);
        
        // Should align "the " ... skip "quick " ... match "brown fox"
        
        // "quick " is 6 chars
        let skips = res.iter().filter(|(op, _, input_char)| *op == EditOp::Substitute && input_char.is_none()).count();
        assert_eq!(skips, 6);
        
        // Ensure "brown" is matched
        let matches = res.iter().filter(|(op, _, _)| *op == EditOp::Match).count();
        // "the " (4) + "brown fox" (9) = 13 matches
        assert_eq!(matches, 13);
    }

    #[test]
    fn test_long_error_string_issue() {
        // Reproducing the "string of errors" issue
        // If I type "th" then skip to "brown", it might try to substitute "e quick" with "brown" 
        // if the cost isn't tuned right.
        let quote = "the quick brown fox";
        let input = "the brown"; 
        
        let res = align_incremental(quote, input);
        
        // Should be:
        // Match: t, h, e, space
        // Skip: q, u, i, c, k, space
        // Match: b, r, o, w, n
        
        let matches: Vec<char> = res.iter()
            .filter(|(op, _, _)| *op == EditOp::Match)
            .map(|(_, q, _)| q.unwrap())
            .collect();
            
        let expected_matches: Vec<char> = "the brown".chars().collect();
        assert_eq!(matches, expected_matches, "Should match 'the brown' correctly");
    }
}
