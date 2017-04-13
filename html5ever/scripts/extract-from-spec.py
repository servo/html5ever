#!/usr/bin/env python
# Copyright 2014 The html5ever Project Developers. See the
# COPYRIGHT file at the top-level directory of this distribution.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

import re
import bs4

# Extract information from the WHATWG webapp spec.

def parse_spec():
    with file('webapps.html') as f:
        soup = bs4.BeautifulSoup(f)

    return {
        'tokenization': soup.find(text='Tokenization').find_parent('div'),
    }

def tokenizer_state_ident(longname):
    longname = longname.lower()
    assert longname[-5:] == 'state'
    words = re.sub(r'[^a-z]', ' ', longname[:-5]).split()
    return ''.join(w.title() for w in words)

def extract_tokenizer_states(spec):
    with file('tokenizer/states.rs', 'w') as f:
        f.write('pub enum State {\n')

        for statedefn in spec['tokenization'].select('h5 > dfn'):
            f.write('    %s,\n' % (tokenizer_state_ident(statedefn.text)))

        f.write('}\n')

def extract_tokenizer_graph(spec):
    with file('build/states.dot', 'w') as f:
        f.write('strict digraph {\n')

        for sec in spec['tokenization'].select('h5'):
            name = sec.text
            if name == 'Tokenizing character references':
                continue
            ident = tokenizer_state_ident(name)

            txt = ''
            for sib in sec.next_siblings:
                if isinstance(sib, bs4.Tag):
                    if sib.name == 'h5':
                        break
                    txt += sib.get_text()
                else:
                    txt += sib

            for edge in re.finditer(r'[sS]witch to the (.* state)', txt):
                f.write('    %s -> %s;\n' % (ident, tokenizer_state_ident(edge.group(1))))

        f.write('}\n')

spec = parse_spec()

# extract_tokenizer_states(spec)  # has manual changes
extract_tokenizer_graph(spec)
