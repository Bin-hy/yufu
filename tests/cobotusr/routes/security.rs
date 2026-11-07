use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

#[tokio::test]
async fn security_t02_postgres_fixtures_are_isolated() {
    let (_first_container, first_url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let (_second_container, second_url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let first = Database::connect(first_url).await.unwrap();
    let second = Database::connect(second_url).await.unwrap();
    for db in [&first, &second] {
        db.execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "CREATE TABLE security_fixture_probe (id INTEGER PRIMARY KEY)".to_owned(),
        ))
        .await
        .unwrap();
    }
    first
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "INSERT INTO security_fixture_probe VALUES (1)".to_owned(),
        ))
        .await
        .unwrap();
    let row = second
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS count FROM security_fixture_probe".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
}
