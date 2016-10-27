#!/usr/bin/python

import os.path
import json
from urllib2 import Request, urlopen


def update(atoms_dir):
    anchors_json = os.path.join(atoms_dir, "anchors.json")
    if os.path.exists(anchors_json):
        print("Using cached anchors.json, remove it to re-download.")
    else:
        # API docs: https://api.csswg.org/shepherd/
        request = Request("https://test.csswg.org/shepherd/api/spec/?anchors&drafts")
        request.add_header("Accept", "application/vnd.csswg.shepherd.v1+json")
        open(anchors_json, "wb").write(urlopen(request).read())
    specs = json.load(open(anchors_json, "rb"))

    local_names = set()

    def traverse(anchors):
        for anchor in anchors:
            traverse(anchor.get("children", []))
            if anchor.get("type") in ("element", "element-attr"):
                linking_text = anchor.get("linking_text")
                if linking_text:
                    identifier = linking_text[0]
                else:
                    identifier = anchor["title"]

                # The data seems to contain some incorrect "element-attr" entries,
                # where `identifier` is a section title rather than an attribute name.
                # "Starts with a lower-case letter" seems to be a good heuristic to filter them out:
                if identifier[0].islower():
                    local_names.add(identifier)

    for spec in specs.itervalues():
        traverse(spec.get("anchors", []))
        traverse(spec.get("draft_anchors", []))

    to_write = "\n".join(sorted(local_names)).encode("utf8")
    open(os.path.join(atoms_dir, "local_names.txt"), "wb").write(to_write)
    print("local_names.txt written.")


if __name__ == "__main__":
    update(os.path.dirname(__file__))
