#![allow(redundant_self)]

use crate::foo::self; //~ ERROR unresolved import `crate::foo`

use std::mem::self;

fn main() {}
