use regex::Regex;
use std::collections::VecDeque;

pub struct Resource(VecDeque<ResourceToken>);

#[derive(Debug)]
enum ResourceToken {
    All,
    Object(String),
    CurlyOpen,
    CurlyClose,
    Comma,
    Slash,
}

impl Resource {
    pub fn from_str(mut str: &str) -> Result<Resource, &str> {
        let mut resp = VecDeque::new();

        let mut curly_open = false;
        let mut word = String::new();
        let reg = Regex::new(r"(^\.{1,2}$|[^a-zA-Z0-9_-]+|.*\.\..*)").unwrap();
        for c in str.chars() {
            match c {
                '/' => resp.push_back(ResourceToken::Slash),
                '*' => resp.push_back(ResourceToken::All),
                '{' => {
                    curly_open = true;
                    resp.push_back(ResourceToken::CurlyOpen);
                }
                '}' => {
                    curly_open = false;
                    if !reg.is_match(&word) {
                        return Err("");
                    }
                    resp.push_back(ResourceToken::Object(word));
                    word = String::new();
                    resp.push_back(ResourceToken::CurlyClose);
                }
                ',' => {
                    if reg.is_match(&word) {
                        return Err("");
                    }
                    resp.push_back(ResourceToken::Object(word));
                    word = String::new();
                    resp.push_back(ResourceToken::Comma)
                }
                ch => {
                    word.push(ch);
                }
            }
        }

        if curly_open {
            return Err("");
        }

        Ok(Resource(resp))
    }

    pub fn to_str(&self) -> String {
        let res = self.0.iter().fold(String::new(), |mut acc, res| {
            match res {
                ResourceToken::All => acc.push('*'),
                ResourceToken::Object(str) => acc.push_str(str),
                ResourceToken::CurlyOpen => acc.push('{'),
                ResourceToken::CurlyClose => acc.push('}'),
                ResourceToken::Comma => acc.push(','),
                ResourceToken::Slash => acc.push('/'),
            }
            acc
        });

        res
    }
}

pub enum ResourceError {
    Invalid,
}
