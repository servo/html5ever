#![feature(plugin)]

#![plugin(string_cache_plugin)]

#[macro_use] extern crate log;
#[macro_use] extern crate mac;
extern crate string_cache;
extern crate tendril;
extern crate time;

macro_rules! time {
    ($e:expr) => {{
        let t0 = ::time::precise_time_ns();
        let result = $e;
        let dt = ::time::precise_time_ns() - t0;
        (result, dt)
    }}
}

#[macro_use] mod util;
pub mod tokenizer;
pub mod tree_builder;
