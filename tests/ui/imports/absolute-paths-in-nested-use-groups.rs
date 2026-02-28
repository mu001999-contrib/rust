#![allow(unused_imports)]

mod foo {}

use foo::{
    ::bar,
    //~^ ERROR: crate root in paths can only be used in start position
    super::bar,
    //~^ ERROR: `super` in paths can only be used in start position
    self::bar,
    //~^ ERROR: unresolved import `foo::self::bar`
};

fn main() {}
