use super::{
    helper::{self, get_sample_ops},
    TestInstruction,
};

#[test]
fn agg_query() {
    let queries = vec![
        r#" 
          SELECT film_id, count(film_id) from film
          GROUP By film_id;
        "#,
        r#"
          SELECT actor_id, count(actor_id) from actor
          GROUP By actor_id;
        "#,
    ];

    helper::compare_with_sqlite(
        &["film", "actor"],
        &queries,
        TestInstruction::FromCsv("actor", vec!["film", "actor"]),
    );
}

#[test]
fn agg_updates_query() {
    let queries = vec![
        r#"
        SELECT actor_id, count(actor_id) from actor
        GROUP By actor_id;
      "#,
    ];

    helper::compare_with_sqlite(
        &["actor"],
        &queries,
        TestInstruction::List(get_sample_ops()),
    );
}
