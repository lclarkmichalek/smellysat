pub(crate) fn sort_and_dedupe<T: Eq + Ord>(vec: &mut Vec<T>) {
    if vec.len() < 2 {
        return;
    }
    vec.sort();

    let mut deduped = 0;
    let mut dupes = 0;
    while deduped < vec.len() - 1 {
        if vec[deduped] == vec[deduped + 1] {
            dupes += 1;
        } else {
            vec.swap(deduped - dupes, deduped);
        }
        deduped += 1;
    }
    vec.truncate(vec.len() - dupes);
}
