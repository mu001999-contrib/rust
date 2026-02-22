#![deny(redundant_self)]

use std::fmt::self; //~ ERROR unnecessary `self`

fn main () {
}
