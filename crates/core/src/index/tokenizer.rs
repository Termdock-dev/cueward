use std::iter::Peekable;
use std::str::CharIndices;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// Han characters have individual positions; Tantivy interprets adjacent query tokens as a phrase.
#[derive(Clone, Default)]
pub(super) struct MixedTokenizer;

pub(super) struct MixedStream<'a> {
    text: &'a str,
    chars: Peekable<CharIndices<'a>>,
    token: Token,
}

fn is_han(ch: char) -> bool {
    matches!(ch as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x323af)
}

impl Tokenizer for MixedTokenizer {
    type TokenStream<'a> = MixedStream<'a>;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> MixedStream<'a> {
        MixedStream {
            text,
            chars: text.char_indices().peekable(),
            token: Token::default(),
        }
    }
}

impl TokenStream for MixedStream<'_> {
    fn advance(&mut self) -> bool {
        for (start, ch) in self.chars.by_ref() {
            if !ch.is_alphanumeric() {
                continue;
            }
            let mut end = start + ch.len_utf8();
            if !is_han(ch) {
                while let Some(&(offset, next)) = self.chars.peek() {
                    if !next.is_alphanumeric() || is_han(next) {
                        break;
                    }
                    self.chars.next();
                    end = offset + next.len_utf8();
                }
            }
            self.token.position = self.token.position.wrapping_add(1);
            self.token.offset_from = start;
            self.token.offset_to = end;
            self.token.text.clear();
            self.token.text.push_str(&self.text[start..end]);
            return true;
        }
        false
    }
    fn token(&self) -> &Token {
        &self.token
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}
