#![cfg_attr(not(test), no_std)]

struct Parser<'a> {
    tokenizer: htmlparser::Tokenizer<'a>,
}

impl<'a> Parser<'a> {
    fn parse(story: &'a str) -> Parser<'a> {
        Parser {
            tokenizer: htmlparser::Tokenizer::from(story),
        }
    }

    fn find_elem(&mut self, tag: &str) {
        for token in self.tokenizer.by_ref() {
            let token = token.expect("Could not read token");

            if let htmlparser::Token::ElementStart { local, .. } = token {
                if local == tag {
                    break;
                }
            }
        }
    }

    // NOTE: this always consumes all the attributes and the opening tag's closing bracket.
    fn find_attr(&mut self, attribute_name: &str) -> Option<&'a str> {
        let mut result: Option<&'a str> = None;
        loop {
            // Always expect a next token (even if it's just ElementEnd
            let token = self.tokenizer.next().unwrap();
            let token = token.expect("Could not read token");

            match token {
                htmlparser::Token::Attribute { local, value, .. } => {
                    if local == attribute_name {
                        result = Some(value.map(|s| s.as_str()).unwrap_or("true"));
                    }
                }

                htmlparser::Token::ElementEnd { .. } => {
                    break;
                }

                _ => {
                    panic!("bad token");
                }
            }
        }

        result
    }

    fn find_elem_by_attr(&mut self, tag: &str, attr_name: &str, attr_val: &str) {
        loop {
            self.find_elem(tag);
            let val = self.find_attr(attr_name);
            if let Some(val) = val {
                if val == attr_val {
                    return;
                }
            }
        }
    }
}

pub fn find_passage_text_by_id<'a>(story: &'a str, passage_id: &str) -> &'a str {
    let mut parser = Parser::parse(story);

    parser.find_elem_by_attr("tw-passagedata", "pid", passage_id);

    let text = parser
        .tokenizer
        .next()
        .expect("No more tokens")
        .expect("Could not read token");

    match text {
        htmlparser::Token::Text { text, .. } => text.as_str(),
        _ => panic!("Bad token"),
    }
}

pub fn find_passage_text_by_name<'a>(story: &'a str, passage_name: &str) -> &'a str {
    let mut parser = Parser::parse(story);

    parser.find_elem_by_attr("tw-passagedata", "name", passage_name);

    let text = parser
        .tokenizer
        .next()
        .expect("No more tokens")
        .expect("Could not read token");

    match text {
        htmlparser::Token::Text { text, .. } => text.as_str(),
        _ => panic!("Bad token"),
    }
}

pub fn find_start_passage_id(story: &str) -> &str {
    let mut parser = Parser::parse(story);

    parser.find_elem("tw-storydata");
    parser.find_attr("startnode").unwrap()
}

pub fn get_n_links(passage: &str) -> usize {
    passage.matches("[[").count()
}

pub struct LinkData<'a> {
    pub label: &'a str,
    pub target: &'a str,
}

pub fn get_link_data<'a>(passage: &'a str, n: usize) -> LinkData<'a> {
    const START: &str = "[[";
    const END: &str = "]]";
    const DELIM: &str = "-&gt;";

    let link_start = passage.match_indices(START).nth(n).unwrap().0;
    let link_end = passage.match_indices(END).nth(n).unwrap().0;

    let link_content = &passage[(link_start + 2)..link_end];
    let link_delim = link_content.find(DELIM);

    let (label, target) = match link_delim {
        None => (link_content, link_content),
        Some(n) => (&link_content[..n], &link_content[n + DELIM.len()..]),
    };

    LinkData { label, target }
}

#[cfg(test)]
mod test {

    use crate::*;

    #[test]
    fn can_find_passage_id() {
        const STORY: &str = r#"
        <tw-storydata startnode="1">
            <tw-passagedata pid="1"></tw-passagedata>
        </tw-storydata>
        "#;
        let passage_id = find_start_passage_id(STORY);
        assert_eq!(passage_id, "1");
    }

    #[test]
    fn can_find_elem_by_attr() {
        const STORY: &str = r#"
        <tw-storydata>
            <tw-passagedata pid="1"></tw-passagedata>
            <tw-passagedata pid="2">Hello</tw-passagedata>
        </tw-storydata>
        "#;

        let mut parser = Parser::parse(&STORY);
        parser.find_elem_by_attr("tw-passagedata", "pid", "2");
        if let Some(next) = parser.tokenizer.next() {
            let token = next.expect("Could not read token");
            match token {
                htmlparser::Token::Text { text } => {
                    assert_eq!(text.as_str(), "Hello");
                }
                _ => {
                    panic!("Unexpected token: {:?}", token);
                }
            }
        } else {
            panic!("No more tokens");
        }
    }

    #[test]
    fn can_find_passage_text_by_id() {
        const STORY: &str = r#"
            <tw-storydata>
                <tw-passagedata pid="1">Once upon a time...</tw-passagedata>
                <tw-passagedata pid="2">The end.</tw-passagedata>
            </tw-storydata>
        "#;

        let text = find_passage_text_by_id(&STORY, "1");
        assert!(text.starts_with("Once upon a time"));

        let text = find_passage_text_by_id(&STORY, "2");
        assert!(text.starts_with("The end"));
    }

    #[test]
    fn can_find_passage_text_by_name() {
        const STORY: &str = r#"
            <tw-storydata>
                <tw-passagedata pid="1" name="intro">Once upon a time...</tw-passagedata>
                <tw-passagedata pid="2" name="end">The end.</tw-passagedata>
            </tw-storydata>
        "#;

        let text = find_passage_text_by_name(&STORY, "intro");
        assert!(text.starts_with("Once upon a time"));

        let text = find_passage_text_by_name(&STORY, "end");
        assert!(text.starts_with("The end"));
    }

    #[test]
    fn can_find_n_links() {
        let n_links = get_n_links("[[Hello]]");
        assert_eq!(n_links, 1);

        let n_links = get_n_links("[[Hello]]\n[[World]]");
        assert_eq!(n_links, 2);
    }

    #[test]
    fn can_find_link_data() {
        let passage = "[[Hello]]\n[[Foo-&gt;Bar]]";

        let link_0 = get_link_data(passage, 0);
        assert_eq!(link_0.label, "Hello");
        assert_eq!(link_0.target, "Hello");

        let link_1 = get_link_data(passage, 1);
        assert_eq!(link_1.label, "Foo");
        assert_eq!(link_1.target, "Bar");
    }

    #[test]
    fn can_find_link_data_story() {
        // Test that passages can be accessed by name or alias
        const STORY: &str = r#"
        <tw-storydata startnode="1">
            <tw-passagedata pid="1">
                Once upon a time...
                [[Continue-&gt;foo]]
            </tw-passagedata>
            <tw-passagedata pid="2" name="foo">
                Other passage
                [[The end]]
            </tw-passagedata>
            <tw-passagedata pid="3" name="The end">
                The end.
            </tw-passagedata>
        </tw-story-data>
"#;
        let start_pid = find_start_passage_id(&STORY);

        let passage = find_passage_text_by_id(&STORY, start_pid);

        let n_links = get_n_links(passage);
        assert_eq!(n_links, 1);

        let link_data = get_link_data(passage, 0);
        let passage = find_passage_text_by_name(&STORY, link_data.target);

        assert!(passage.trim().starts_with("Other passage"));

        let link_data = get_link_data(passage, 0);
        let passage = find_passage_text_by_name(&STORY, link_data.target);

        assert_eq!(passage.trim(), "The end.");
    }
}
