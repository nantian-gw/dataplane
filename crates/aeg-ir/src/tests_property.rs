use proptest::{collection::vec as prop_vec, prelude::*, string::string_regex};

include!("tests_property/request_meta.rs");
include!("tests_property/grpc_selection.rs");
include!("tests_property/proto_snapshot.rs");

fn grpc_identifier_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z][A-Za-z0-9]{0,7}")
        .expect("grpc identifier regex")
        .boxed()
}

fn path_segment_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z0-9._~-]{1,16}")
        .expect("path segment regex")
        .boxed()
}

fn method_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z]{1,8}").expect("method regex").boxed()
}

fn mixed_case_token_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z][A-Za-z0-9]{0,7}")
        .expect("mixed case token regex")
        .boxed()
}

fn header_name_strategy() -> BoxedStrategy<String> {
    string_regex("[a-z][a-z0-9-]{0,10}")
        .expect("header name regex")
        .boxed()
}

fn header_value_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z0-9._~-]{1,16}")
        .expect("header value regex")
        .boxed()
}

fn query_value_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z0-9._~-]{0,12}")
        .expect("query value regex")
        .boxed()
}

fn resource_name_strategy() -> BoxedStrategy<String> {
    string_regex("[a-z][a-z0-9-]{0,12}")
        .expect("resource name regex")
        .boxed()
}

fn hostname_strategy() -> BoxedStrategy<String> {
    prop_vec(resource_name_strategy(), 1..4)
        .prop_map(|segments: Vec<String>| segments.join("."))
        .boxed()
}
