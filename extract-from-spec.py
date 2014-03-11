#!/usr/bin/env python
import re
from bs4 import BeautifulSoup

# Extract information from the WHATWG webapp spec.

def parse_spec():
    with file('webapps.html') as f:
        soup = BeautifulSoup(f)

    return {
        'tokenization': soup.find(text='Tokenization').find_parent('div'),
    }

def extract_tokenizer_states(spec):
    with file('tokenizer/states.rs', 'w') as f:
        f.write('pub enum State {\n');

        for statedefn in spec['tokenization'].select('h5 > dfn'):
            statename = statedefn.text.lower()
            assert statename[-5:] == 'state'
            words = re.sub(r'[^a-z]', ' ', statename[:-5]).split()
            f.write('    %s,\n' % (''.join(w.title() for w in words),))

        f.write('}\n')

spec = parse_spec()
extract_tokenizer_states(spec)
