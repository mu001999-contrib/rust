//@ revisions: e2015 e2018
//@ [e2015] edition: 2015
//@ [e2018] edition: 2018..

#![deny(redundant_self)]

pub mod x {
    pub mod y {
        pub use crate::self; //~ ERROR imports need to be explicitly named
        //~^ ERROR unnecessary `self`
        pub use crate::self as crate1;
        //~^ ERROR unnecessary `self`
        pub use crate::{self}; //~ ERROR imports need to be explicitly named
        //~^ ERROR unnecessary `self`
        pub use crate::{self, x as x1}; //~ ERROR imports need to be explicitly named
        pub use crate::{self as crate2};
        //~^ ERROR unnecessary `self`
        pub use crate::{self as crate3, x as x2};

        pub use self; //~ ERROR imports need to be explicitly named
        pub use self as self1;
        pub use {self}; //~ ERROR imports need to be explicitly named
        pub use {self as self2};
        pub use self::self as self3;
        //~^ ERROR unnecessary `self`
        pub use self::{self}; //~ ERROR imports need to be explicitly named
        //~^ ERROR unnecessary `self`
        pub use self::{self, yy as yy1}; //~ ERROR imports need to be explicitly named
        pub use self::{self as self4};
        //~^ ERROR unnecessary `self`
        pub use self::{self as self5, yy as yy2};

        pub use super::self; //~ ERROR imports need to be explicitly named
        //~^ ERROR unnecessary `self`
        pub use super::self as super1;
        //~^ ERROR unnecessary `self`
        pub use super::{self}; //~ ERROR imports need to be explicitly named
        //~^ ERROR unnecessary `self`
        pub use super::{self, z as z1}; //~ ERROR imports need to be explicitly named
        pub use super::{self as super2};
        //~^ ERROR unnecessary `self`
        pub use super::{self as super3, z as z2};

        pub use crate::x::self;
        //~^ ERROR unnecessary `self`
        pub use crate::x::self as x3;
        //~^ ERROR unnecessary `self`
        pub use crate::x::{self}; //~ ERROR the name `x` is defined multiple times
        //~^ ERROR unnecessary `self`
        pub use crate::x::{self, z as z3}; //~ ERROR the name `x` is defined multiple times
        pub use crate::x::{self as x4};
        //~^ ERROR unnecessary `self`
        pub use crate::x::{self as x5, z as z4};

        pub use ::self; //[e2018]~ ERROR extern prelude cannot be imported
        //[e2015]~^ ERROR imports need to be explicitly named
        pub use ::self as crate4; //[e2018]~ ERROR extern prelude cannot be imported
        pub use ::{self}; //[e2018]~ ERROR extern prelude cannot be imported
        //[e2015]~^ ERROR imports need to be explicitly named
        pub use ::{self as crate5}; //[e2018]~ ERROR extern prelude cannot be imported

        pub mod yy {}
    }

    pub mod z {}
}

fn main() {}
