mod modifiers;
#[cfg(test)]
mod tests;

use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitChar, LitStr, Token, bracketed, parenthesized};

use crate::models::{RenderType, TokenParserState, WeldToken};
use crate::parser::modifiers::parse_modifiers;

pub(crate) const MAX_DEPTH: isize = 20;

#[derive(Debug)]
pub struct TweldDsl {
    pub render_type: RenderType,
    pub tokens: Vec<WeldToken>,
}

impl Parse for TweldDsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut dsl = TweldDsl {
            render_type: RenderType::Identifier,
            tokens: Vec::new(),
        };
        
        dsl = parse_stream(input, dsl, TokenParserState::Root, -1isize)?;

        Ok(dsl)
    }
}

fn parse_stream(
    input: &syn::parse::ParseBuffer<'_>,
    mut dsl: TweldDsl,
    state: TokenParserState,
    mut depth: isize,
) -> syn::Result<TweldDsl> {
    if depth >= MAX_DEPTH {
        return Err(syn::Error::new(input.span(), "Maximum nesting exceeded!"));
    }
    depth += 1;

    let mut words: Vec<String> = vec![];
    while !input.is_empty() {
        match state {
            TokenParserState::Root => {
                if input.peek(syn::token::Paren) {
                    if !words.is_empty() {
                        parse_words(&mut dsl, &state, &mut words);
                    }
                    parse_concat_group(input, &mut dsl, depth)?;
                    continue;
                }

                if input.peek(syn::token::Bracket) {
                    if !words.is_empty() {
                        parse_words(&mut dsl, &state, &mut words);
                    }
                    parse_list_group(input, &mut dsl, depth)?;
                    continue;
                }

                if input.peek(Token![|]) {
                    dsl = parse_stream(input, dsl, TokenParserState::Modifiers, depth)?;
                    continue;
                }

                if input.peek(syn::Ident) {
                    let result = input.parse::<Ident>()?.to_string();
                    dsl.tokens.push(WeldToken::Plain(result));
                    continue;
                }

                if input.peek(syn::LitStr) {
                    let result = input.parse::<LitStr>()?.value();
                    dsl.render_type = RenderType::StringLiteral;
                    dsl.tokens.push(WeldToken::Plain(result));
                    continue;
                }

                if input.peek(syn::LitChar) {
                    let result = input.parse::<LitChar>()?.value().to_string();
                    dsl.render_type = RenderType::StringLiteral;
                    dsl.tokens.push(WeldToken::Plain(result));
                    continue;
                }

                if input.peek(Token![-]) {
                    input.parse::<Token![-]>()?;
                    dsl.tokens.push(WeldToken::Plain("-".to_string()));
                    continue;
                }

                if input.peek(Token![_]) {
                    input.parse::<Token![_]>()?;
                    dsl.tokens.push(WeldToken::Plain("_".to_string()));
                    continue;
                }

                let _ = input.parse::<proc_macro2::TokenTree>()?;
            }
            TokenParserState::InsideGroup(_) => {
                while !input.is_empty() {
                    if input.peek(syn::token::Paren) {
                        if !words.is_empty() {
                            parse_words(&mut dsl, &state, &mut words);
                        }
                        parse_concat_group(input, &mut dsl, depth)?;

                        continue;
                    }

                    if input.peek(syn::token::Bracket) {
                        if !words.is_empty() {
                            parse_words(&mut dsl, &state, &mut words);
                        }
                        parse_list_group(input, &mut dsl, depth)?;

                        continue;
                    }

                    if input.peek(Token![|]) {
                        if !words.is_empty() {
                            let Some(last) = words.pop() else {
                                return Err(syn::Error::new(
                                    input.span(),
                                    "There was an error when trying to start modifiers...",
                                ));
                            };

                            if !words.is_empty() {
                                dsl.tokens.push(WeldToken::Plain(words.join("")));
                                words.clear();
                            }
                            dsl.tokens.push(WeldToken::Plain(last));
                        }

                        dsl = parse_stream(input, dsl, TokenParserState::Modifiers, depth)?;
                        continue;
                    }

                    if input.peek(syn::Ident) {
                        let value = input.parse::<Ident>()?.to_string();

                        words.push(value);
                        continue;
                    }

                    if input.peek(syn::LitChar) {
                        let value = input.parse::<LitChar>()?.value();

                        words.push(value.to_string());
                        dsl.render_type = RenderType::StringLiteral;
                        continue;
                    }

                    if input.peek(syn::LitStr) {
                        let value = input.parse::<LitStr>()?.value();

                        words.push(value);
                        dsl.render_type = RenderType::StringLiteral;
                        continue;
                    }

                    if input.peek(Token![-]) {
                        input.parse::<Token![-]>()?;
                        words.push("-".to_string());
                        continue;
                    }

                    if input.peek(Token![_]) {
                        input.parse::<Token![_]>()?;
                        words.push("_".to_string());
                        continue;
                    }

                    let _ = input.parse::<proc_macro2::TokenTree>()?;
                }
            }
            TokenParserState::Modifiers => {
                let Some(target) = dsl.tokens.pop() else {
                    return Err(syn::Error::new(input.span(), "Modifiers need a target"));
                };

                let modifiers = parse_modifiers(input)?;
                dsl.tokens
                    .push(WeldToken::Modify(Box::new(target), modifiers));
            }
        }

        if !words.is_empty() {
            parse_words(&mut dsl, &state, &mut words);
        }
    }
    if !words.is_empty() {
        parse_words(&mut dsl, &state, &mut words);
    }

    Ok(dsl)
}

fn parse_words(dsl: &mut TweldDsl, state: &TokenParserState, words: &mut Vec<String>) {
    match state {
        TokenParserState::InsideGroup(single_value) if !single_value => {
            words
                .iter()
                .for_each(|word| dsl.tokens.push(WeldToken::Plain(word.clone())));
        }
        _ => dsl.tokens.push(WeldToken::Plain(words.join(""))),
    }

    words.clear();
}

fn parse_concat_group(
    input: &syn::parse::ParseBuffer<'_>,
    dsl: &mut TweldDsl,
    depth: isize,
) -> Result<(), syn::Error> {
    let group;
    parenthesized!(group in input);
    let mut grouped_dsl = TweldDsl {
        render_type: dsl.render_type.clone(),
        tokens: vec![],
    };
    grouped_dsl = parse_stream(
        &group,
        grouped_dsl,
        TokenParserState::InsideGroup(true),
        depth,
    )?;

    dsl.tokens.push(WeldToken::ConcatGroup(grouped_dsl.tokens));

    if let RenderType::StringLiteral = grouped_dsl.render_type {
        dsl.render_type = RenderType::StringLiteral;
    }

    Ok(())
}

fn parse_list_group(
    input: &syn::parse::ParseBuffer<'_>,
    dsl: &mut TweldDsl,
    depth: isize,
) -> Result<(), syn::Error> {
    let group;
    bracketed!(group in input);
    let mut grouped_dsl = TweldDsl {
        render_type: dsl.render_type.clone(),
        tokens: vec![],
    };
    grouped_dsl = parse_stream(
        &group,
        grouped_dsl,
        TokenParserState::InsideGroup(false),
        depth,
    )?;

    dsl.tokens.push(WeldToken::ListGroup(grouped_dsl.tokens));

    if let RenderType::StringLiteral = grouped_dsl.render_type {
        dsl.render_type = RenderType::StringLiteral;
    }

    Ok(())
}
