use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::mem::take;
use std::rc::Rc;
use std::sync::LazyLock;
use std::vec;

use regex::{Captures, Regex};

#[derive(Debug, Clone)]
pub enum AttrValue {
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub tag: String,
    pub attrs: HashMap<String, AttrValue>,
    pub children: Vec<Element>,
}

impl Element {
    pub fn get_attr_str(&self, key: &str) -> Option<&str> {
        match self.attrs.get(key) {
            Some(AttrValue::Str(value)) => Some(value),
            _ => None,
        }
    }

    pub fn get_attr_bool(&self, key: &str) -> Option<bool> {
        match self.attrs.get(key) {
            Some(AttrValue::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn get_text(&self) -> Option<&str> {
        if self.tag == "text" {
            Some(self.get_attr_str("content").unwrap_or(""))
        } else {
            None
        }
    }

    fn fmt_attrs(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (key, value) in &self.attrs {
            f.write_str(" ")?;
            match value {
                AttrValue::Bool(true) => f.write_str(key)?,
                AttrValue::Bool(false) => {
                    f.write_str("no-")?;
                    f.write_str(key)?;
                }
                AttrValue::Str(value) => {
                    f.write_str(key)?;
                    f.write_str("=\"")?;
                    f.write_str(&escape(value, true))?;
                    f.write_str("\"")?;
                }
            }
        }
        Ok(())
    }

    pub fn strip(&self) -> String {
        if self.tag == "text" {
            self.get_attr_str("content").unwrap_or("").to_string()
        } else {
            let mut out = String::new();
            for child in &self.children {
                out += &child.strip();
            }
            out
        }
    }

    pub fn text(content: String) -> Self {
        let mut attrs = HashMap::new();
        attrs.insert("content".to_string(), AttrValue::Str(content));
        Self {
            tag: "text".to_string(),
            attrs,
            children: Vec::new(),
        }
    }
}

impl Display for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.tag == "text" {
            f.write_str(&escape(self.get_attr_str("content").unwrap_or(""), false))?;
        } else if self.children.is_empty() {
            f.write_str("<")?;
            f.write_str(&self.tag)?;
            self.fmt_attrs(f)?;
            f.write_str("/>")?;
        } else {
            f.write_str("<")?;
            f.write_str(&self.tag)?;
            self.fmt_attrs(f)?;
            f.write_str(">")?;
            for child in &self.children {
                child.fmt(f)?;
            }
            f.write_str("</")?;
            f.write_str(&self.tag)?;
            f.write_str(">")?;
        }
        Ok(())
    }
}

pub fn dump(elements: &[Element]) -> String {
    let mut out = String::new();
    for element in elements {
        out += &element.to_string();
    }
    out
}

pub fn strip(elements: &[Element]) -> String {
    let mut out = String::new();
    for element in elements {
        out += &element.strip();
    }
    out
}

static TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<comment><!--[\s\S]*?-->)|(?<tag><(\/?)([^!\s>/]*)([^>]*?)\s*(\/?)>)").unwrap()
});
static ATTR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([^\s=]+)(?:="(?<value1>[^"]*)"|='(?<value2>[^']*)')?"#).unwrap()
});
static UNESCAPE_DEC_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#(\d+);").unwrap());
static UNESCAPE_HEX_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&#x([0-9a-f]+);").unwrap());
static UNESCAPE_AMP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&(amp|#38|#x26);").unwrap());
static TRIM_START_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\n\s*").unwrap());
static TRIM_END_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\n\s*$").unwrap());

pub fn escape(src: &str, inline: bool) -> String {
    let src = src
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;");
    if inline {
        src.replace("\"", "&quot;")
    } else {
        src
    }
}

pub fn unescape(src: &str) -> String {
    let src = src
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    let src = UNESCAPE_DEC_REGEX.replace_all(&src, |cap: &Captures| {
        let code = cap
            .get(1)
            .unwrap()
            .as_str()
            .parse::<u32>()
            .unwrap_or(0xfffd);
        if code == 38 {
            cap.get_match().as_str().to_string()
        } else {
            char::from_u32(code).unwrap_or('\u{fffd}').to_string()
        }
    });
    let src = UNESCAPE_HEX_REGEX.replace_all(&src, |cap: &Captures| {
        let code = u32::from_str_radix(cap.get(1).unwrap().as_str(), 16).unwrap_or(0xfffd);
        if code == 38 {
            cap.get_match().as_str().to_string()
        } else {
            char::from_u32(code).unwrap_or('\u{fffd}').to_string()
        }
    });
    let src = UNESCAPE_AMP_REGEX.replace_all(&src, "&");
    src.to_string()
}

#[derive(Debug, Clone)]
enum TokenPosition {
    Open,
    Close,
    Empty,
}

#[derive(Debug, Clone)]
struct Token {
    name: String,
    position: TokenPosition,
    extra: String,
}

#[derive(Debug, Clone)]
enum TokenLike {
    Str(String),
    Token(Token),
}

#[derive(Debug, Clone)]
struct TokenTree {
    token: Token,
    children: Vec<TokenTreeLike>,
}

#[derive(Debug, Clone)]
enum TokenTreeLike {
    Str(String),
    Token(Rc<RefCell<TokenTree>>),
}

pub fn parse(src: &str) -> Option<Vec<Element>> {
    let lexed = lex_tokens(src);
    let folded = fold_tokens(lexed)?;
    let parsed = parse_tokens(folded);
    Some(parsed)
}

fn lex_tokens(mut src: &str) -> Vec<TokenLike> {
    let mut tokens = Vec::<TokenLike>::new();
    while let Some(tag_match) = TAG_REGEX.captures(src) {
        let content = unescape(&src[0..tag_match.get_match().start()]);
        let content = TRIM_START_REGEX.replace(&content, "");
        let content = TRIM_END_REGEX.replace(&content, "");
        if !content.is_empty() {
            tokens.push(TokenLike::Str(content.to_string()));
        }

        // TODO use Regex.captures_iter
        src = &src[tag_match.get_match().end()..];
        if tag_match.name("comment").is_some() {
            continue;
        }
        let close = tag_match.get(3).unwrap();
        let tag = tag_match.get(4).unwrap();
        let extra = tag_match.get(5).unwrap();
        let empty = tag_match.get(6).unwrap();
        tokens.push(TokenLike::Token(Token {
            name: tag.as_str().to_string(),
            position: if !close.is_empty() {
                TokenPosition::Close
            } else if !empty.is_empty() {
                TokenPosition::Empty
            } else {
                TokenPosition::Open
            },
            extra: extra.as_str().to_string(),
        }));
    }

    let content = unescape(src);
    let content = TRIM_START_REGEX.replace(&content, "");
    let content = TRIM_END_REGEX.replace(&content, "");
    if !content.is_empty() {
        tokens.push(TokenLike::Str(content.to_string()));
    }

    tokens
}

fn fold_tokens(tokens: Vec<TokenLike>) -> Option<Vec<TokenTreeLike>> {
    let mut stack = vec![Rc::new(RefCell::new(TokenTree {
        token: Token {
            name: "".to_string(),
            position: TokenPosition::Open,
            extra: "".to_string(),
        },
        children: Vec::new(),
    }))];

    for token in tokens {
        match token {
            TokenLike::Str(token) => {
                stack[0]
                    .borrow_mut()
                    .children
                    .push(TokenTreeLike::Str(token));
            }
            TokenLike::Token(token) => match token.position {
                TokenPosition::Close => {
                    if stack[0].borrow().token.name == token.name {
                        stack.remove(0);
                    }
                }
                TokenPosition::Open => {
                    let tree = Rc::new(RefCell::new(TokenTree {
                        token,
                        children: Vec::new(),
                    }));
                    stack[0]
                        .borrow_mut()
                        .children
                        .push(TokenTreeLike::Token(tree.clone()));
                    stack.insert(0, tree);
                }
                TokenPosition::Empty => {
                    stack[0]
                        .borrow_mut()
                        .children
                        .push(TokenTreeLike::Token(Rc::new(RefCell::new(TokenTree {
                            token,
                            children: Vec::new(),
                        }))));
                }
            },
        }
    }

    return Some(stack.last()?.borrow().children.clone());
}

fn parse_tokens(tokens: Vec<TokenTreeLike>) -> Vec<Element> {
    let mut result = Vec::<Element>::new();

    for token in tokens {
        match token {
            TokenTreeLike::Str(content) => result.push(Element::text(content)),
            TokenTreeLike::Token(token) => {
                let mut token = token.borrow_mut();
                let mut attrs = HashMap::new();
                for attr_match in ATTR_REGEX.captures_iter(&token.token.extra) {
                    let key = attr_match.get(1).unwrap().as_str();
                    let value = attr_match
                        .name("value1")
                        .or_else(|| attr_match.name("value2"));
                    match value {
                        Some(value) => {
                            attrs.insert(key.to_string(), AttrValue::Str(unescape(value.as_str())));
                        }
                        None => {
                            if let Some(key) = key.strip_prefix("no-") {
                                attrs.insert(key.to_string(), AttrValue::Bool(false));
                            } else {
                                attrs.insert(key.to_string(), AttrValue::Bool(true));
                            }
                        }
                    }
                }
                result.push(Element {
                    tag: take(&mut token.token.name),
                    attrs,
                    children: parse_tokens(take(&mut token.children)),
                })
            }
        }
    }

    result
}
