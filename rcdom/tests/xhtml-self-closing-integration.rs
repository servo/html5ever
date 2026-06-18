//! Integration tests for the `xhtml-self-closing` feature.
//!
//! EPUB content is XHTML and uses self-closing syntax on RCDATA/RAWTEXT
//! elements such as `<title/>` and `<style/>`. Without `xhtml-self-closing`,
//! html5ever treats these as opening tags and enters raw-text mode, consuming
//! the rest of the document

#[cfg(feature = "xhtml-self-closing")]
mod xhtml_self_closing {
    use html5ever::driver;
    use html5ever::tendril::stream::TendrilSink;
    use html5ever::tendril::StrTendril;
    use markup5ever_rcdom::{NodeData, RcDom};

    fn parse(input: &str) -> RcDom {
        driver::parse_document(RcDom::default(), Default::default()).one(StrTendril::from(input))
    }

    /// Walk the tree and collect all element names.
    fn element_names(node: &markup5ever_rcdom::Handle) -> Vec<String> {
        let mut names = Vec::new();
        collect_names(node, &mut names);
        names
    }

    fn collect_names(node: &markup5ever_rcdom::Handle, out: &mut Vec<String>) {
        if let NodeData::Element { ref name, .. } = node.data {
            out.push(name.local.to_string());
        }
        for child in node.children.borrow().iter() {
            collect_names(child, out);
        }
    }

    /// Return the text content of the first element with the given local name.
    fn text_of(dom: &RcDom, tag: &str) -> Option<String> {
        find_text(&dom.document, tag)
    }

    fn find_text(node: &markup5ever_rcdom::Handle, tag: &str) -> Option<String> {
        if let NodeData::Element { ref name, .. } = node.data {
            if name.local.as_ref() == tag {
                let mut text = String::new();
                for child in node.children.borrow().iter() {
                    if let NodeData::Text { ref contents } = child.data {
                        text.push_str(&contents.borrow());
                    }
                }
                return Some(text);
            }
        }
        for child in node.children.borrow().iter() {
            if let Some(t) = find_text(child, tag) {
                return Some(t);
            }
        }
        None
    }

    #[test]
    fn self_closing_title_does_not_swallow_body() {
        let dom = parse("<html><head><title/></head><body><p>visible</p></body></html>");
        let names = element_names(&dom.document);

        assert!(
            names.contains(&"body".to_string()),
            "body element should be present; got: {:?}",
            names
        );
        assert!(
            names.contains(&"p".to_string()),
            "p element inside body should be present; got: {:?}",
            names
        );

        let text = text_of(&dom, "p");
        assert_eq!(
            text.as_deref(),
            Some("visible"),
            "<p> text should be 'visible', got: {:?}",
            text
        );
    }

    #[test]
    fn self_closing_style_does_not_swallow_body() {
        let dom = parse("<html><head><style/></head><body><p>content</p></body></html>");
        let names = element_names(&dom.document);

        assert!(
            names.contains(&"p".to_string()),
            "p element should not be swallowed by <style/>; got: {:?}",
            names
        );
    }

    #[test]
    fn normal_closed_title_still_captures_rcdata_text() {
        let dom = parse("<html><head><title>My Book</title></head><body></body></html>");
        let text = text_of(&dom, "title");
        assert_eq!(
            text.as_deref(),
            Some("My Book"),
            "title text should be 'My Book', got: {:?}",
            text
        );
    }
}
