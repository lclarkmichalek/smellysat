use crate::instance::Literal;

/// Sort and deduplicate a vector of Literals using SIMD-optimized deduplication.
/// This is the hot path version optimized with sosorted.
pub(crate) fn sort_and_dedupe_literals(vec: &mut Vec<Literal>) {
    if vec.len() < 2 {
        return;
    }
    vec.sort();

    // Use sosorted's SIMD-optimized deduplicate on the underlying u32 data
    let as_u32 = Literal::slice_as_u32_mut(vec.as_mut_slice());
    let new_len = sosorted::deduplicate(as_u32);
    vec.truncate(new_len);
}

/// Generic sort and deduplicate for any Ord + Eq type.
/// Used for types that aren't Literal.
#[allow(dead_code)]
pub(crate) fn sort_and_dedupe<T: Eq + Ord>(vec: &mut Vec<T>) {
    if vec.len() < 2 {
        return;
    }
    vec.sort();
    vec.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::Variable;

    #[test]
    fn test_sort_and_dedupe_literals() {
        let v0 = Variable(0);
        let v1 = Variable(1);
        let v2 = Variable(2);

        let mut lits = vec![
            Literal::new(v2, true),
            Literal::new(v0, true),
            Literal::new(v1, true),
            Literal::new(v0, true), // duplicate
            Literal::new(v1, true), // duplicate
        ];

        sort_and_dedupe_literals(&mut lits);

        assert_eq!(lits.len(), 3);
        assert_eq!(lits[0], Literal::new(v0, true));
        assert_eq!(lits[1], Literal::new(v1, true));
        assert_eq!(lits[2], Literal::new(v2, true));
    }

    #[test]
    fn test_sort_and_dedupe_literals_empty() {
        let mut lits: Vec<Literal> = vec![];
        sort_and_dedupe_literals(&mut lits);
        assert!(lits.is_empty());
    }

    #[test]
    fn test_sort_and_dedupe_literals_single() {
        let mut lits = vec![Literal::new(Variable(0), true)];
        sort_and_dedupe_literals(&mut lits);
        assert_eq!(lits.len(), 1);
    }

    #[test]
    fn test_sort_and_dedupe_generic() {
        let mut nums = vec![3, 1, 2, 1, 3, 2];
        sort_and_dedupe(&mut nums);
        assert_eq!(nums, vec![1, 2, 3]);
    }
}
