pub fn private_conversation_id(user1: i64, user2: i64) -> i64 {
    if user1 < user2 {
        user1 * 1_000_000 + user2
    } else {
        user2 * 1_000_000 + user1
    }
}
