use elara_http::smoke_eval_one_plus_one;

#[test]
fn links_libelara_and_evaluates_one_plus_one() {
    smoke_eval_one_plus_one().expect("Elara smoke eval");
}
