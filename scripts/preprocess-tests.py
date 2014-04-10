#!/usr/bin/env python
#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.
#
# Work around mozilla/rust#13064 by reserializing test JSON files
# without Unicode \uXXXX escapes.

import os
import sys
import codecs
import simplejson
from os import path

src_dir, dst_dir = sys.argv[1:]

subdir = path.split(dst_dir)[1]

test_dir = path.join(src_dir, subdir)
for filename in os.listdir(test_dir):
    if not filename.endswith('.test'):
        continue

    with file(path.join(test_dir, filename), 'r') as infile:
        js = simplejson.load(infile)

    with file(path.join(dst_dir, filename), 'w') as outfile:
        simplejson.dump(js, codecs.getwriter('utf8')(outfile), ensure_ascii=False)
