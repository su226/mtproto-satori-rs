use std::{cell::RefCell, collections::HashMap, fmt::Display, rc::Rc, vec};

use lazy_static::lazy_static;
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
            Some(self.get_attr_str("content").unwrap_or(&""))
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

pub fn dump(elements: Vec<Element>) -> String {
    let mut out = String::new();
    for element in elements {
        out += &element.to_string();
    }
    out
}

pub fn strip(elements: Vec<Element>) -> String {
    let mut out = String::new();
    for element in elements {
        out += &element.strip();
    }
    out
}

lazy_static! {
    static ref TAG_REGEX: Regex =
        Regex::new(r"(?<comment><!--[\s\S]*?-->)|(?<tag><(\/?)([^!\s>/]*)([^>]*?)\s*(\/?)>)")
            .unwrap();
    static ref ATTR_REGEX: Regex =
        Regex::new(r#"([^\s=]+)(?:="(?<value1>[^"]*)"|='(?<value2>[^']*)')?"#).unwrap();
    static ref UNESCAPE_DEC_REGEX: Regex = Regex::new(r"&#(\d+);").unwrap();
    static ref UNESCAPE_HEX_REGEX: Regex = Regex::new(r"&#x([0-9a-f]+);").unwrap();
    static ref UNESCAPE_AMP_REGEX: Regex = Regex::new(r"&(amp|#38|#x26);").unwrap();
    static ref TRIM_START_REGEX: Regex = Regex::new(r"^\s*\n\s*").unwrap();
    static ref TRIM_END_REGEX: Regex = Regex::new(r"\s*\n\s*$").unwrap();
}

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
    let parsed = parse_tokens(&folded);
    Some(parsed)
}

fn lex_tokens(mut src: &str) -> Vec<TokenLike> {
    let mut tokens = Vec::<TokenLike>::new();
    // TODO remove trim_start and trim_end as they always true (without "curly" token)?
    let mut trim_start = true;
    while let Some(tag_match) = TAG_REGEX.captures(src) {
        let trim_end = true;

        let content = unescape(&src[0..tag_match.get_match().start()]);
        debug_assert!(trim_start);
        let content = if trim_start {
            TRIM_START_REGEX.replace(&content, "").to_string()
        } else {
            content
        };
        debug_assert!(trim_end);
        let content = if trim_end {
            TRIM_END_REGEX.replace(&content, "").to_string()
        } else {
            content
        };
        if !content.is_empty() {
            tokens.push(TokenLike::Str(content));
        }

        trim_start = trim_end;
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
    debug_assert!(trim_start);
    let content = if trim_start {
        TRIM_START_REGEX.replace(&content, "").to_string()
    } else {
        content
    };
    let content = TRIM_END_REGEX.replace(&content, "").to_string();
    if !content.is_empty() {
        tokens.push(TokenLike::Str(content));
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

fn parse_tokens(tokens: &[TokenTreeLike]) -> Vec<Element> {
    let mut result = Vec::<Element>::new();

    for token in tokens {
        match token {
            TokenTreeLike::Str(content) => result.push(Element::text(content.to_string())),
            TokenTreeLike::Token(token) => {
                let token = token.borrow();
                let mut attrs = HashMap::new();
                let mut extra = token.token.extra.as_ref();
                while let Some(attr_match) = ATTR_REGEX.captures(extra) {
                    let key = attr_match.get(1).unwrap().as_str().to_string();
                    let value = attr_match
                        .name("value1")
                        .or_else(|| attr_match.name("value2"));
                    match value {
                        Some(value) => {
                            attrs.insert(key, AttrValue::Str(unescape(value.as_str())));
                        }
                        None => {
                            if key.starts_with("no-") {
                                attrs.insert(key[3..].to_string(), AttrValue::Bool(false));
                            } else {
                                attrs.insert(key, AttrValue::Bool(true));
                            }
                        }
                    }
                    extra = &extra[attr_match.get_match().end()..];
                }
                result.push(Element {
                    tag: token.token.name.clone(),
                    attrs,
                    children: parse_tokens(&token.children),
                })
            }
        }
    }

    result
}
