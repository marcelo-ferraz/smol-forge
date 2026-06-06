use proc_macro2::{Delimiter, Group, TokenTree};
use proc_macro2::{Literal, TokenStream};
use syn::{parse_str, parse2};

use crate::builder::build_string;
use crate::models::RenderType;
use crate::parser::TweldDsl;

pub(crate) const IDENT_EMPTY_MSG: &str = "The identifier is an empty string!";

fn contains_at(stream: &TokenStream) -> bool {
    for token in stream.clone().into_iter() {
        match token {
            TokenTree::Punct(ref p) if p.as_char() == '@' => return true,
            TokenTree::Group(g) => {
                if contains_at(&g.stream()) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

pub fn scan_tokens(input: TokenStream) -> syn::Result<TokenStream> {
    if !input
        .clone()
        .into_iter()
        .any(|t| matches!(t, TokenTree::Punct(ref p) if p.as_char() == '@'))
    {
        return Ok(input);
    }

    let mut output = Vec::new();
    let mut tokens = input.into_iter().peekable();

    while let Some(tree) = tokens.next() {
        match tree {
            // Checking for the '@' hook
            TokenTree::Punct(ref p) if p.as_char() == '@' => {
                if let Some(TokenTree::Group(grp)) = tokens.peek() {
                    let span = grp.span();
                    if grp.delimiter() == Delimiter::Bracket {
                        // We found @[ ... ]! Consume the bracket group.
                        let Some(TokenTree::Group(bracket_group)) = tokens.next() else {
                            return Err(syn::Error::new(
                                span,
                                "There was an error when consuming the bracket group!",
                            ));
                        };

                        let dsl: TweldDsl = parse2(bracket_group.stream())?;

                        let result = build_string(dsl.tokens, &dsl.render_type);

                        match dsl.render_type {
                            RenderType::Identifier => {
                                let result = result.replace(" ", "");

                                if result.is_empty() {
                                    return Err(syn::Error::new(span, IDENT_EMPTY_MSG));
                                }

                                let identifier =
                                    parse_str::<proc_macro2::Ident>(&result).or_else(|_| {
                                        parse_str::<proc_macro2::Ident>(&format!("r#{result}"))
                                    })?;

                                output.push(TokenTree::Ident(identifier));
                            }
                            RenderType::StringLiteral => {
                                output.push(TokenTree::Literal(Literal::string(&result)));
                            }
                        }

                        continue;
                    }
                }
                output.push(tree);
            }

            TokenTree::Group(g) => {
                if contains_at(&g.stream()) {
                    let inner_expanded = scan_tokens(g.stream())?;
                    let mut new_group = Group::new(g.delimiter(), inner_expanded);
                    new_group.set_span(g.span());
                    output.push(TokenTree::Group(new_group));
                } else {
                    output.push(TokenTree::Group(g));
                }
            }

            t => {
                output.push(t);
            }
        }
    }
    Ok(TokenStream::from_iter(output))
}
