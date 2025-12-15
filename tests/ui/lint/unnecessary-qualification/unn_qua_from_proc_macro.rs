//@ edition: 2021
//@ proc-macro: make-unn-qua.rs
//@ aux-build: unn-qua-helper.rs

#![deny(unused_qualifications)]

extern crate make_unn_qua;
extern crate unn_qua_helper;

use unn_qua_helper::a::b;

#[make_unn_qua::bad_unn_qua]
fn bar() {
    if true {}
}

fn main() {
    b::foo();
    bar();
}
