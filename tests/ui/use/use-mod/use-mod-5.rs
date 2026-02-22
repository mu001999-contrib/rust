//@ check-pass

#![allow(redundant_self)]

mod foo {
    pub mod bar {
        pub fn drop() {}
    }
}

use foo::bar::self;

fn main() {
    // Because of error recovery this shouldn't error
    bar::drop();
}
