# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

#!/usr/bin/env python
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
