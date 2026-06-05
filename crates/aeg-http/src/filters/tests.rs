use aeg_ir::{
    CorsFilter, Filter, HeaderModifier, HeaderOperation, MatchedHttpPath, PathModifier,
    RequestRedirectFilter, UrlRewriteFilter,
};
use pingora::http::{RequestHeader, ResponseHeader};
use proptest::{prelude::*, string::string_regex};
use std::collections::BTreeMap;

mod cors;
mod header_and_request;
mod redirect_and_support;

fn filter_value_strategy() -> BoxedStrategy<String> {
    string_regex("[A-Za-z0-9._~-]{1,16}")
        .expect("filter value regex")
        .boxed()
}

fn host_strategy() -> BoxedStrategy<String> {
    string_regex("[a-z][a-z0-9]{0,8}(\\.[a-z][a-z0-9]{0,8}){1,2}")
        .expect("host regex")
        .boxed()
}

fn rewrite_prefix_strategy() -> BoxedStrategy<String> {
    string_regex("/[a-z0-9]{1,8}")
        .expect("rewrite prefix regex")
        .boxed()
}
