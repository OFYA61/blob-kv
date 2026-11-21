use chumsky::{prelude::*, text::ascii::keyword};

#[derive(Debug, PartialEq)]
pub enum Command {
    Get { key: String },
    Set { key: String, value: Option<String> },
    Del { key: String },
}

impl Command {
    pub fn from(str: &str) -> Result<Command, Vec<String>> {
        parser()
            .parse(str)
            .into_result()
            // TODO: improve error messages
            .map_err(|err| err.to_vec().iter().map(|e| e.to_string()).collect())
    }
}

fn parser<'src>() -> impl Parser<'src, &'src str, Command, extra::Err<Rich<'src, char>>> {
    let unquoted_value = any()
        .filter(|c: &char| !c.is_whitespace() && *c != '"' && *c != '\'')
        .repeated()
        .at_least(1)
        .to_slice()
        .map(str::to_string);
    let single_quoted_value = any()
        .filter(|c: &char| *c != '\'' && *c != '\n' && *c != '\r')
        .repeated()
        .to_slice()
        .map(str::to_string)
        .delimited_by(just('\''), just('\''));
    let double_quoted_value = any()
        .filter(|c: &char| *c != '"' && *c != '\n' && *c != '\r')
        .repeated()
        .to_slice()
        .map(str::to_string)
        .delimited_by(just('"'), just('"'));

    let value = choice((single_quoted_value, double_quoted_value, unquoted_value)).padded();

    let get = keyword("GET")
        .ignore_then(value)
        .map(|key| Command::Get { key });
    let set = keyword("SET")
        .ignore_then(value.then(value.or_not()))
        .map(|(key, value)| Command::Set { key, value });
    let del = keyword("DEL")
        .ignore_then(value)
        .map(|key| Command::Del { key });

    choice((get, set, del)).padded().then_ignore(end())
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse(input: &str) -> Result<Command, Vec<Rich<'_, char>>> {
        parser().parse(input).into_result()
    }

    fn assert_rejected(input: &str) {
        assert!(parse(input).is_err(), "expected {input:?} to be rejected");
    }

    #[test]
    fn get() {
        assert_eq!(
            parse("GET hello"),
            Ok(Command::Get {
                key: "hello".to_string()
            })
        );
    }

    #[test]
    fn set_with_value() {
        assert_eq!(
            parse("SET hello world"),
            Ok(Command::Set {
                key: "hello".to_string(),
                value: Some("world".to_string())
            })
        );
    }

    #[test]
    fn set_without_value() {
        assert_eq!(
            parse("SET hello"),
            Ok(Command::Set {
                key: "hello".to_string(),
                value: None
            })
        );
    }

    #[test]
    fn del() {
        assert_eq!(
            parse("DEL hello"),
            Ok(Command::Del {
                key: "hello".to_string()
            })
        );
    }

    #[test]
    fn get_with_quoted_key() {
        assert_eq!(
            parse(r#"GET "hello world""#),
            Ok(Command::Get {
                key: "hello world".to_string()
            })
        );
        assert_eq!(
            parse("GET 'hello world'"),
            Ok(Command::Get {
                key: "hello world".to_string()
            })
        );
    }

    #[test]
    fn del_with_quoted_key() {
        assert_eq!(
            parse(r#"DEL "hello world""#),
            Ok(Command::Del {
                key: "hello world".to_string()
            })
        );
        assert_eq!(
            parse("DEL 'hello world'"),
            Ok(Command::Del {
                key: "hello world".to_string()
            })
        );
    }

    #[test]
    fn set_with_quoted_key_and_value() {
        assert_eq!(
            parse(r#"SET "hello world" "some value""#),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: Some("some value".to_string())
            })
        );
        assert_eq!(
            parse("SET 'hello world' 'some value'"),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: Some("some value".to_string())
            })
        );
    }

    #[test]
    fn set_with_mixed_quote_styles() {
        assert_eq!(
            parse(r#"SET "hello world" 'some value'"#),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: Some("some value".to_string())
            })
        );
        assert_eq!(
            parse("SET 'hello world' \"some value\""),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: Some("some value".to_string())
            })
        );
    }

    #[test]
    fn set_with_quoted_key_and_no_value() {
        assert_eq!(
            parse(r#"SET "hello world""#),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: None
            })
        );
        assert_eq!(
            parse("SET 'hello world'"),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: None
            })
        );
    }

    #[test]
    fn quoted_values_can_contain_the_other_quote() {
        assert_eq!(
            parse(r#"SET key "it's fine""#),
            Ok(Command::Set {
                key: "key".to_string(),
                value: Some("it's fine".to_string())
            })
        );
        assert_eq!(
            parse("SET key 'he said \"hi\"'"),
            Ok(Command::Set {
                key: "key".to_string(),
                value: Some("he said \"hi\"".to_string())
            })
        );
    }

    #[test]
    fn empty_quoted_strings_are_preserved() {
        assert_eq!(parse(r#"GET """#), Ok(Command::Get { key: String::new() }));
        assert_eq!(parse("GET ''"), Ok(Command::Get { key: String::new() }));
        assert_eq!(
            parse(r#"SET key """#),
            Ok(Command::Set {
                key: "key".to_string(),
                value: Some(String::new())
            })
        );
        assert_eq!(
            parse("SET key ''"),
            Ok(Command::Set {
                key: "key".to_string(),
                value: Some(String::new())
            })
        );
    }

    #[test]
    fn surrounding_whitespace_around_quoted_values_is_accepted() {
        assert_eq!(
            parse(r#"  SET   "hello world"   "some value"  "#),
            Ok(Command::Set {
                key: "hello world".to_string(),
                value: Some("some value".to_string())
            })
        );
    }

    #[test]
    fn unclosed_quotes_are_rejected() {
        assert_rejected(r#"GET "hello"#);
        assert_rejected("GET 'hello");
        assert_rejected(r#"SET "hello" "world"#);
        assert_rejected("SET 'hello' 'world");
    }

    #[test]
    fn mismatched_quotes_are_rejected() {
        assert_rejected(r#"GET "hello'"#);
        assert_rejected("GET 'hello\"");
    }

    #[test]
    fn quoted_values_cannot_span_lines() {
        assert_rejected("GET \"hello\nworld\"");
        assert_rejected("GET 'hello\nworld'");
    }

    #[test]
    fn surrounding_and_repeated_whitespace_is_accepted() {
        assert_eq!(
            parse(" \t GET   hello \n"),
            Ok(Command::Get {
                key: "hello".to_string()
            })
        );
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_rejected("");
        assert_rejected("   ");
    }

    #[test]
    fn unknown_command_is_rejected() {
        assert_rejected("NOPE hello");
    }

    #[test]
    fn missing_key_is_rejected() {
        assert_rejected("GET");
        assert_rejected("SET");
        assert_rejected("DEL");
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_rejected("GET hello world");
        assert_rejected("SET hello world extra");
        assert_rejected("DEL hello world");
    }

    #[test]
    fn commands_must_match_whole_keyword() {
        assert_rejected("GETTER hello");
        assert_rejected("SETTINGS hello");
        assert_rejected("DELETE hello");
        assert_rejected("GEThello");
    }
}
