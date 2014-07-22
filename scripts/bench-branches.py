#!/usr/bin/env python
# Copyright 2014 The html5ever Project Developers. See the
# COPYRIGHT file at the top-level directory of this distribution.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

import os
import sys
import simplejson
import subprocess

if not os.getcwd().endswith('/build'):
    sys.stderr.write('Run me from the build directory')
    sys.exit(1)

branches = sys.argv[1:]

# Prefixing a branch name with '=' means don't re-run that benchmark.
branches_run = [b for b in branches if not b.startswith('=')]
branches = [b.lstrip('=') for b in branches]

baseline = branches[0]

for branch in branches_run:
    subprocess.check_call(
        '''../configure &&
         git checkout {0:s} &&
         BENCH_UNCOMMITTED=1 make RUSTFLAGS="-O" METRICS=metrics.{0:s}.json clean bench \
            | tee bench.{0:s}'''
        .format(branch), shell=True)

data = {}
for branch in branches:
    with file('metrics.{:s}.json'.format(branch)) as f:
        data[branch] = simplejson.load(f)

keys = data[data.iterkeys().next()].keys()
for branch, dat in data.iteritems():
    if branch == baseline:
        continue
    for k in keys:
        old = data[baseline][k]['value']
        new = dat[k]['value']
        chg = (new - old) / float(old)
        desc = 'worse'
        if chg < 0:
            desc = 'better'
            chg = -chg

        print '{:50s}: {:8s} {:6s} by {:5.1f}%'.format(
            k, branch, desc, 100*chg)

    print
