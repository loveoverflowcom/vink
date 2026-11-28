//! CommonMark AST to a closed set of Leptos nodes. The same projection runs in SSR and WASM.
use leptos::prelude::*;
use url::Url;
use vink_markdown_text::{Event, HeadingLevel, Tag, commonmark};
use vink_ui::{Heading, TypeRole};

// Bound both the Rust/DOM recursion and the indentation of very deeply nested manuscripts.
// Fall back to literal presentation, with the source H1 still in its position.
const MAX_NESTING: usize = 24;
const DESCRIPTION_CHARS: usize = 160;

pub(super) struct Markdown {
    nodes: Vec<Node>,
    pub(super) has_h1: bool,
}

enum Node {
    Text(String),
    RawHtml(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Rule,
    Group(Element, Vec<Node>),
}

enum Element {
    Paragraph,
    Title(usize),
    CompactHeading,
    Heading(HeadingLevel, usize),
    Quote,
    CodeBlock,
    HtmlBlock,
    BulletList,
    OrderedList(u64),
    Item,
    Emphasis,
    Strong,
    Link(Option<String>),
    Image,
}

impl Markdown {
    pub(super) fn parse(source: &str) -> Self {
        let mut heading_count = 0;
        let mut has_h1 = false;
        match parse_nodes(
            &mut commonmark(source),
            0,
            false,
            &mut heading_count,
            &mut has_h1,
        ) {
            Some(nodes) => Self { nodes, has_h1 },
            None => Self::literal_fallback(source),
        }
    }

    fn literal_fallback(source: &str) -> Self {
        let heading = commonmark(source)
            .into_offset_iter()
            .filter_map(|(event, range)| match event {
                Event::Start(Tag::Heading { level, .. }) => Some((level, range)),
                _ => None,
            })
            .enumerate()
            .find(|(_, (level, _))| *level == HeadingLevel::H1);
        let verbatim =
            |text: &str| Node::Group(Element::HtmlBlock, vec![Node::Text(text.to_owned())]);
        let nodes = match &heading {
            Some((index, (_, range))) => vec![
                verbatim(&source[..range.start]),
                Node::Group(Element::Title(index + 1), Vec::new()),
                verbatim(&source[range.end..]),
            ],
            None => vec![verbatim(source)],
        };
        Self {
            nodes,
            has_h1: heading.is_some(),
        }
    }

    pub(super) fn description(&self) -> String {
        description(&self.nodes).unwrap_or_default()
    }

    pub(super) fn render(self, title: &str) -> AnyView {
        let content = render_nodes(self.nodes, title);
        view! { <div class="vk-document vk-markdown" data-format="markdown">{content}</div> }
            .into_any()
    }
}

pub(crate) fn render_comment(source: String) -> AnyView {
    let nodes = parse_nodes(&mut commonmark(&source), 0, true, &mut 0, &mut false)
        .unwrap_or_else(|| vec![Node::Group(Element::HtmlBlock, vec![Node::Text(source)])]);
    let children = render_nodes(nodes, "");
    view! { <div class="vk-discussion-body vk-comment-markdown">{children}</div> }.into_any()
}

fn parse_nodes<'a>(
    events: &mut impl Iterator<Item = Event<'a>>,
    depth: usize,
    compact: bool,
    heading_count: &mut usize,
    has_h1: &mut bool,
) -> Option<Vec<Node>> {
    let mut nodes = Vec::new();
    while let Some(event) = events.next() {
        let node = match event {
            Event::Start(tag) => {
                if depth == MAX_NESTING {
                    return None;
                }
                let element = match tag {
                    Tag::Paragraph => Element::Paragraph,
                    Tag::Heading { .. } if compact => Element::CompactHeading,
                    Tag::Heading { level, .. } => {
                        *heading_count += 1;
                        if level == HeadingLevel::H1 && !*has_h1 {
                            *has_h1 = true;
                            Element::Title(*heading_count)
                        } else {
                            Element::Heading(level, *heading_count)
                        }
                    }
                    Tag::BlockQuote(_) => Element::Quote,
                    Tag::CodeBlock(_) => Element::CodeBlock,
                    Tag::HtmlBlock => Element::HtmlBlock,
                    Tag::List(None) => Element::BulletList,
                    Tag::List(Some(start)) => Element::OrderedList(start),
                    Tag::Item => Element::Item,
                    Tag::Emphasis => Element::Emphasis,
                    Tag::Strong => Element::Strong,
                    Tag::Link { dest_url, .. } => Element::Link(safe_href(&dest_url)),
                    Tag::Image { .. } => Element::Image,
                    // Extension-only tags have no mapping. If exposed by a parser change, fail
                    // closed to the literal projection.
                    _ => return None,
                };
                Node::Group(
                    element,
                    parse_nodes(events, depth + 1, compact, heading_count, has_h1)?,
                )
            }
            Event::End(_) => break,
            Event::Text(text) => Node::Text(text.into_string()),
            Event::Code(code) => Node::Code(code.into_string()),
            Event::Html(html) | Event::InlineHtml(html) => Node::RawHtml(html.into_string()),
            Event::SoftBreak => Node::SoftBreak,
            Event::HardBreak => Node::HardBreak,
            Event::Rule => Node::Rule,
            _ => return None,
        };
        nodes.push(node);
    }
    Some(nodes)
}

/// Only authored navigation links are allowed. Reject ambiguous paths before URL normalization.
/// Image destinations are never passed here, and no authored URL is used as a resource source.
fn safe_href(href: &str) -> Option<String> {
    if href
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return None;
    }
    if (href.starts_with('#') && href.len() > 1)
        || (href.starts_with('/') && !href.starts_with("//"))
    {
        return Some(href.to_owned());
    }
    let (scheme, rest) = href.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
        || rest.starts_with(['/', '?', '#'])
    {
        return None;
    }
    let url = Url::parse(href).ok()?;
    (url.host().is_some() && url.username().is_empty() && url.password().is_none())
        .then(|| url.into())
}

fn plain_text(nodes: &[Node], output: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) | Node::Code(text) => output.push_str(text),
            Node::SoftBreak | Node::HardBreak => output.push('\n'),
            Node::Group(Element::HtmlBlock, _) | Node::RawHtml(_) | Node::Rule => {}
            Node::Group(_, children) => plain_text(children, output),
        }
    }
}

fn description(nodes: &[Node]) -> Option<String> {
    nodes.iter().find_map(|node| match node {
        Node::Group(Element::Paragraph, children) => {
            let mut text = String::new();
            plain_text(children, &mut text);
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| line.chars().take(DESCRIPTION_CHARS).collect())
        }
        Node::Group(
            Element::Quote | Element::BulletList | Element::OrderedList(_) | Element::Item,
            children,
        ) => description(children),
        _ => None,
    })
}

fn render_nodes(nodes: Vec<Node>, title: &str) -> Vec<AnyView> {
    nodes.into_iter().map(|node| node.render(title)).collect()
}

impl Node {
    fn render(self, title: &str) -> AnyView {
        match self {
            Self::Text(text) | Self::RawHtml(text) => text.into_any(),
            Self::Code(code) => view! { <code>{code}</code> }.into_any(),
            Self::SoftBreak => "\n".into_any(),
            Self::HardBreak => view! { <br/> }.into_any(),
            Self::Rule => view! { <hr/> }.into_any(),
            Self::Group(element, nodes) => element.render(nodes, title),
        }
    }
}

impl Element {
    fn render(self, nodes: Vec<Node>, title: &str) -> AnyView {
        match self {
            Self::Title(index) => {
                let title = title.to_owned();
                view! { <Heading level=vink_ui::HeadingLevel::H1 role=TypeRole::Display id=heading_id(index)>{title}</Heading> }.into_any()
            }
            Self::Image => {
                let mut alt = String::new();
                plain_text(&nodes, &mut alt);
                if alt.trim().is_empty() {
                    alt = "Ảnh".into();
                }
                view! { <span class="vk-markdown__image">{alt}</span> }.into_any()
            }
            Self::Link(href) => {
                let mut label = String::new();
                plain_text(&nodes, &mut label);
                let children = render_nodes(nodes, title);
                match href.filter(|_| !label.trim().is_empty()) {
                    Some(href) => {
                        view! { <a href=href rel="ugc noreferrer">{children}</a> }.into_any()
                    }
                    None => children.into_any(),
                }
            }
            element => {
                let children = render_nodes(nodes, title);
                match element {
                    Self::CompactHeading => view! { <p><strong>{children}</strong></p> }.into_any(),
                    Self::Paragraph => view! { <p>{children}</p> }.into_any(),
                    Self::Heading(level, index) => heading(level, index, children),
                    Self::Quote => view! { <blockquote>{children}</blockquote> }.into_any(),
                    Self::CodeBlock => {
                        view! { <pre tabindex="0"><code>{children}</code></pre> }.into_any()
                    }
                    Self::HtmlBlock => {
                        view! { <div class="vk-markdown__literal">{children}</div> }.into_any()
                    }
                    Self::BulletList => view! { <ul>{children}</ul> }.into_any(),
                    Self::OrderedList(start) => {
                        view! { <ol start=start>{children}</ol> }.into_any()
                    }
                    Self::Item => view! { <li>{children}</li> }.into_any(),
                    Self::Emphasis => view! { <em>{children}</em> }.into_any(),
                    Self::Strong => view! { <strong>{children}</strong> }.into_any(),
                    Self::Title(_) | Self::Image | Self::Link(_) => unreachable!(),
                }
            }
        }
    }
}

fn heading_id(index: usize) -> String {
    format!("muc-{index}")
}

fn heading(level: HeadingLevel, index: usize, children: Vec<AnyView>) -> AnyView {
    let id = heading_id(index);
    match level {
        HeadingLevel::H1 => view! { <h1 id=id>{children}</h1> }.into_any(),
        HeadingLevel::H2 => view! { <h2 id=id>{children}</h2> }.into_any(),
        HeadingLevel::H3 => view! { <h3 id=id>{children}</h3> }.into_any(),
        HeadingLevel::H4 => view! { <h4 id=id>{children}</h4> }.into_any(),
        HeadingLevel::H5 => view! { <h5 id=id>{children}</h5> }.into_any(),
        HeadingLevel::H6 => view! { <h6 id=id>{children}</h6> }.into_any(),
    }
}
