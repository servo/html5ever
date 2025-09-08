#!/usr/bin/env python

from os.path import join, dirname
import re
from urllib.request import urlopen

print("Fetching https://drafts.csswg.org/css-counter-styles...")
counter_styles_spec = urlopen("https://drafts.csswg.org/css-counter-styles")

print("Finding counter styles")
names = [];
for line in counter_styles_spec:
  if b'data-dfn-for="<counter-style-name>"' in line or b'data-dfn-for="<counter-style>"' in line:
    counter_style = re.search('>([^>]+)(</dfn>|<a class="self-link")', line.decode()).group(1)
    names.append(counter_style)

filename = join(dirname(__file__), "predefined_counter_styles.txt")
with open(filename, "w") as f:
  for name in names:
    f.write(f"{name}\n")

print("Done.")
