// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ffi::OsStr;
use std::fs;
use std::ops::FnMut;
use std::path::Path;

pub fn foreach_html5lib_test<Mk>(
    src_dir: &Path,
    subdir: &'static str,
    ext: &'static OsStr,
    mut mk: Mk,
) where
    Mk: FnMut(&Path, fs::File),
{
    let mut test_dir_path = src_dir.to_path_buf();
    test_dir_path.push("html5lib-tests");
    test_dir_path.push(subdir);

    let maybe_test_files = fs::read_dir(&test_dir_path);
    match maybe_test_files {
        Ok(test_files) => {
            for entry in test_files {
                let path = entry.unwrap().path();
                if path.extension() == Some(ext) {
                    let file = fs::File::open(&path).unwrap();
                    mk(&path, file);
                }
            }
        },
        Err(_) => {
            panic!("Before launching the tests, please run this command:\n\n\tgit submodule update --init\n\nto retrieve an html5lib-tests snapshot.");
        },
    }
}
