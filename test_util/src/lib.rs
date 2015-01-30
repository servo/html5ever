// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(io, path)]

use std::old_io as io;
use std::old_path::{GenericPath,Path};
use std::ops::FnMut;
use std::str::StrExt;

pub fn foreach_html5lib_test<Mk>(
        src_dir: Path,
        subdir: &'static str,
        ext: &'static str,
        mut mk: Mk)
    where Mk: FnMut(&str, io::File)
{
    let test_dir_path = src_dir.join_many(&["html5lib-tests", subdir]);
    let test_files = io::fs::readdir(&test_dir_path).unwrap();
    for path in test_files.into_iter() {
        let path_str = path.filename_str().unwrap();
        if path_str.ends_with(ext) {
            let file = io::File::open(&path).unwrap();
            mk(path_str, file);
        }
    }
}
