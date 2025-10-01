#![allow(unused_mut)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![feature(dropck_eyepatch)] // permanently unstable feature

pub mod async_await;
pub mod atomics;
pub mod cell;
pub mod channels;
pub mod dropck;
pub mod lifetimes;
pub mod macros;
pub mod rc;
pub mod refcell;
pub mod variance;
