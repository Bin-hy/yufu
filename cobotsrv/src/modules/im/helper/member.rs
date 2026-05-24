pub fn normalize_member_ids(member_ids: Vec<i64>) -> Vec<i64> {
    let mut ids = member_ids
        .into_iter()
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
}
