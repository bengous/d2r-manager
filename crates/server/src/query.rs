use crate::failure::{Failure, Kind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Query {
    pairs: Vec<(String, String)>,
}

impl Query {
    pub(crate) fn parse(text: &str, allowed: &[&str]) -> Result<Self, Failure> {
        let mut pairs = Vec::new();
        for field in text.split('&').filter(|field| !field.is_empty()) {
            let (name, value) = field.split_once('=').unwrap_or((field, ""));
            let name = decode(name)?;
            if !allowed.contains(&name.as_str()) {
                return Err(Failure::new(
                    Kind::BadRequest,
                    format!(
                        "the query holds no parameter named {name:?}, only {}",
                        allowed.join(", ")
                    ),
                ));
            }
            if pairs
                .iter()
                .any(|(held, _): &(String, String)| *held == name)
            {
                return Err(Failure::new(
                    Kind::BadRequest,
                    format!("the parameter {name:?} is given twice"),
                ));
            }
            pairs.push((name, decode(value)?));
        }
        Ok(Self { pairs })
    }

    pub(crate) fn optional(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn required(&self, name: &str) -> Result<&str, Failure> {
        self.optional(name).ok_or_else(|| {
            Failure::new(
                Kind::BadRequest,
                format!("the query carries no {name} parameter"),
            )
        })
    }
}

fn decode(text: &str) -> Result<String, Failure> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' => {
                let digits = bytes
                    .get(index + 1..index + 3)
                    .and_then(|pair| std::str::from_utf8(pair).ok())
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                    .ok_or_else(|| {
                        Failure::new(
                            Kind::BadRequest,
                            format!(
                                "{text:?} holds a percent sign no two hexadecimal digits follow"
                            ),
                        )
                    })?;
                decoded.push(digits);
                index += 2;
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8(decoded).map_err(|_| {
        Failure::new(
            Kind::BadRequest,
            format!("{text:?} decodes to bytes no text holds"),
        )
    })
}
