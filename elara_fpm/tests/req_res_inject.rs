use elara_fpm::smoke_inject_req_res;

#[test]
fn injects_req_and_res_into_script_wrapper() {
    smoke_inject_req_res().expect("req/res injection smoke");
}
