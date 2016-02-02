#!/usr/bin/env python
# Copyright 2015 The html5ever Project Developers. See the
# COPYRIGHT file at the top-level directory of this distribution.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

import re
import sys
import subprocess


REPLACEMENTS = {
    'ok': '.',
    'FAILED': 'F',
    'ignored': 'I',
}
TEST_RESULT_RE = re.compile(
    r'^test .* \.\.\. ({0})$'.format('|'.join(REPLACEMENTS.keys())))


def main(args):
    process = subprocess.Popen(args, stdout=subprocess.PIPE)
    while True:
        line = process.stdout.readline()
        if len(line) is 0:
            return process.wait()
        match = TEST_RESULT_RE.match(line)
        if match:
            sys.stdout.write(REPLACEMENTS[match.group(1)])
        else:
            sys.stdout.write(line)
        sys.stdout.flush()


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
