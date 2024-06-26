//! `μfmt` macros

#![warn(warnings)]

extern crate proc_macro;

use core::mem;
use proc_macro::TokenStream;
use std::{borrow::Cow, cmp::Ordering};

use proc_macro2::{Literal, Span};
use quote::quote;
use syn::{
    parse::{self, Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    Data, DeriveInput, Expr, Fields, GenericParam, Ident, LitStr, Token,
};

/// Automatically derive the `uDebug` trait for a `struct` or `enum`
///
/// Supported items
///
/// - all kind of `struct`-s
/// - all kind of `enum`-s
///
/// `union`-s are not supported
#[proc_macro_derive(uDebug)]
pub fn debug(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let mut generics = input.generics;

    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(parse_quote!(vfmt::uDebug));
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let ident = &input.ident;
    let ts = match input.data {
        Data::Struct(data) => {
            let ident_s = ident.to_string();

            let body = match data.fields {
                Fields::Named(fields) => {
                    let fields = fields
                        .named
                        .iter()
                        .map(|field| {
                            let ident = field.ident.as_ref().expect("UNREACHABLE");
                            let name = ident.to_string();

                            quote!(field(#name, &self.#ident)?)
                        })
                        .collect::<Vec<_>>();

                    quote!(f.debug_struct(#ident_s)?#(.#fields)*.finish())
                }

                Fields::Unnamed(fields) => {
                    let fields = (0..fields.unnamed.len())
                        .map(|i| {
                            let i = Literal::u64_unsuffixed(i as u64);

                            quote!(field(&self.#i)?)
                        })
                        .collect::<Vec<_>>();

                    quote!(f.debug_tuple(#ident_s)?#(.#fields)*.finish())
                }

                Fields::Unit => quote!(f.write_str(#ident_s)),
            };

            quote!(
                impl #impl_generics vfmt::uDebug for #ident #ty_generics #where_clause {
                    fn fmt<W>(&self, f: &mut vfmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
                    where
                        W: vfmt::uWrite + ?Sized,
                    {
                        #body
                    }
                }

            )
        }

        Data::Enum(data) => {
            let arms = data
                .variants
                .iter()
                .map(|var| {
                    let variant = &var.ident;
                    let variant_s = variant.to_string();

                    match &var.fields {
                        Fields::Named(fields) => {
                            let mut pats = Vec::with_capacity(fields.named.len());
                            let mut methods = Vec::with_capacity(fields.named.len());
                            for field in &fields.named {
                                let ident = field.ident.as_ref().unwrap();
                                let ident_s = ident.to_string();

                                pats.push(quote!(#ident));
                                methods.push(quote!(field(#ident_s, #ident)?));
                            }

                            quote!(
                                #ident::#variant { #(#pats),* } => {
                                    f.debug_struct(#variant_s)?#(.#methods)*.finish()
                                }
                            )
                        }

                        Fields::Unnamed(fields) => {
                            let pats = &(0..fields.unnamed.len())
                                .map(|i| Ident::new(&format!("_{}", i), Span::call_site()))
                                .collect::<Vec<_>>();

                            quote!(
                                #ident::#variant(#(#pats),*) => {
                                    f.debug_tuple(#variant_s)?#(.field(#pats)?)*.finish()
                                }
                            )
                        }

                        Fields::Unit => quote!(
                            #ident::#variant => {
                                f.write_str(#variant_s)
                            }
                        ),
                    }
                })
                .collect::<Vec<_>>();

            quote!(
                impl #impl_generics vfmt::uDebug for #ident #ty_generics #where_clause {
                    fn fmt<W>(&self, f: &mut vfmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
                        where
                        W: vfmt::uWrite + ?Sized,
                    {
                        match self {
                            #(#arms),*
                        }
                    }
                }
            )
        }

        Data::Union(..) => {
            return parse::Error::new(Span::call_site(), "this trait cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    ts.into()
}

#[proc_macro]
pub fn uwrite(input: TokenStream) -> TokenStream {
    write(input, false)
}

#[proc_macro]
pub fn uwriteln(input: TokenStream) -> TokenStream {
    write(input, true)
}

fn write(input: TokenStream, newline: bool) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    let formatter = &input.formatter;
    let literal = input.literal;

    let mut format = literal.value();
    if newline {
        format.push('\n');
    }
    let pieces = match parse(&format, literal.span()) {
        Err(e) => return e.to_compile_error().into(),
        Ok(pieces) => pieces,
    };

    let required_args = pieces.iter().filter(|piece| !piece.is_str()).count();
    let supplied_args = input.args.len();
    match supplied_args.cmp(&required_args) {
        Ordering::Less => {
            return parse::Error::new(
                literal.span(),
                format!(
                    "format string requires {} arguments but {} {} supplied",
                    required_args,
                    supplied_args,
                    if supplied_args == 1 { "was" } else { "were" }
                ),
            )
            .to_compile_error()
            .into()
        }
        Ordering::Equal => {}
        Ordering::Greater => {
            return parse::Error::new(input.args[required_args].span(), "argument never used")
                .to_compile_error()
                .into()
        }
    }

    let mut args = vec![];
    let mut pats = vec![];
    let mut exprs = vec![];
    let mut i = 0;
    for piece in pieces {
        if let Piece::Str(s) = piece {
            exprs.push(quote!(f.write_str(#s)?;))
        } else {
            let pat = mk_ident(i);
            let arg = &input.args[i];
            i += 1;

            args.push(quote!(&(#arg)));
            pats.push(quote!(#pat));

            match piece {
                Piece::Display => {
                    exprs.push(quote!(vfmt::uDisplay::fmt(#pat, f)?;));
                }

                Piece::Debug { pretty } => {
                    exprs.push(if pretty {
                        quote!(f.pretty(|f| vfmt::uDebug::fmt(#pat, f))?;)
                    } else {
                        quote!(vfmt::uDebug::fmt(#pat, f)?;)
                    });
                }

                Piece::PadSpace { amount } => {
                    exprs.push(
                        quote!(f.pad_right_spaces(#amount - (#pat.len()), |f| vfmt::uDisplay::fmt(#pat, f))?;),
                    );
                }

                _ => unreachable!(),
            }
        }
    }

    quote!(match (#(#args),*) {
        (#(#pats),*) => {
            use vfmt::UnstableDoAsFormatter as _;

            (#formatter).do_as_formatter(|f| {
                #(#exprs)*
                Ok(())
            })
        }
    })
    .into()
}

struct Input {
    formatter: Expr,
    _comma: Token![,],
    literal: LitStr,
    _comma2: Option<Token![,]>,
    args: Punctuated<Expr, Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let formatter = input.parse()?;
        let _comma = input.parse()?;
        let literal = input.parse()?;

        if input.is_empty() {
            Ok(Input {
                formatter,
                _comma,
                literal,
                _comma2: None,
                args: Punctuated::new(),
            })
        } else {
            Ok(Input {
                formatter,
                _comma,
                literal,
                _comma2: input.parse()?,
                args: Punctuated::parse_terminated(input)?,
            })
        }
    }
}

#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Debug { pretty: bool },
    PadSpace { amount: usize },
    Display,
    Str(Cow<'a, str>),
}

impl Piece<'_> {
    fn is_str(&self) -> bool {
        matches!(self, Piece::Str(_))
    }
}

fn mk_ident(i: usize) -> Ident {
    Ident::new(&format!("__{}", i), Span::call_site())
}

// `}}` -> `}`
fn unescape(mut literal: &str, span: Span) -> parse::Result<Cow<'_, str>> {
    if literal.contains('}') {
        let mut buf = String::new();

        while literal.contains('}') {
            const ERR: &str = "format string contains an unmatched right brace";
            let mut parts = literal.splitn(2, '}');

            match (parts.next(), parts.next()) {
                (Some(left), Some(right)) => {
                    const ESCAPED_BRACE: &str = "}";

                    if right.starts_with(ESCAPED_BRACE) {
                        buf.push_str(left);
                        buf.push('}');

                        literal = right.trim_start_matches(ESCAPED_BRACE);
                    } else {
                        return Err(parse::Error::new(span, ERR));
                    }
                }

                _ => unreachable!(),
            }
        }

        buf.push_str(literal);

        Ok(buf.into())
    } else {
        Ok(Cow::Borrowed(literal))
    }
}

fn parse(mut literal: &str, span: Span) -> parse::Result<Vec<Piece<'_>>> {
    let mut pieces = vec![];

    let mut buf = String::new();
    loop {
        let mut parts = literal.splitn(2, '{');
        match (parts.next(), parts.next()) {
            // empty string literal
            (None, None) => break,

            // end of the string literal
            (Some(s), None) => {
                if buf.is_empty() {
                    if !s.is_empty() {
                        pieces.push(Piece::Str(unescape(s, span)?));
                    }
                } else {
                    buf.push_str(&unescape(s, span)?);

                    pieces.push(Piece::Str(Cow::Owned(buf)));
                }

                break;
            }

            (head, Some(tail)) => {
                const DEBUG: &str = ":?}";
                const DEBUG_PRETTY: &str = ":#?}";
                const PAD_SPACE: &str = ": <";
                const DISPLAY: &str = "}";
                const ESCAPED_BRACE: &str = "{";

                let head = head.unwrap_or("");
                if tail.starts_with(DEBUG)
                    || tail.starts_with(DEBUG_PRETTY)
                    || tail.starts_with(PAD_SPACE)
                    || tail.starts_with(DISPLAY)
                {
                    if buf.is_empty() {
                        if !head.is_empty() {
                            pieces.push(Piece::Str(unescape(head, span)?));
                        }
                    } else {
                        buf.push_str(&unescape(head, span)?);

                        pieces.push(Piece::Str(Cow::Owned(mem::take(&mut buf))));
                    }

                    if tail.starts_with(DEBUG) {
                        pieces.push(Piece::Debug { pretty: false });

                        literal = tail.trim_start_matches(DEBUG);
                    } else if tail.starts_with(DEBUG_PRETTY) {
                        pieces.push(Piece::Debug { pretty: true });

                        literal = tail.trim_start_matches(DEBUG_PRETTY);
                    } else if tail.starts_with(PAD_SPACE) {
                        literal = tail.trim_start_matches(PAD_SPACE);
                        let (amount, tail) = literal.split_at(literal.find('}').unwrap_or(0));
                        literal = tail;

                        let amount: usize = amount.parse().map_err(|err| {
                            parse::Error::new(span, format!("pad size but be within a u8: {}", err))
                        })?;

                        pieces.push(Piece::PadSpace { amount });

                        literal = literal.trim_start_matches('}');
                    } else {
                        pieces.push(Piece::Display);

                        literal = &tail[DISPLAY.len()..];
                    }
                } else if tail.starts_with(ESCAPED_BRACE) {
                    buf.push_str(&unescape(head, span)?);
                    buf.push('{');

                    literal = tail.trim_start_matches(ESCAPED_BRACE);
                } else {
                    return Err(parse::Error::new(
                        span,
                        "invalid format string: expected `{{`, `{}`, `{:?}` or `{:#?}`",
                    ));
                }
            }
        }
    }

    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use proc_macro2::Span;

    use crate::Piece;

    #[test]
    fn pieces() {
        let span = Span::call_site();

        // string interpolation
        assert_eq!(
            super::parse("The answer is {}", span).ok(),
            Some(vec![
                Piece::Str(Cow::Borrowed("The answer is ")),
                Piece::Display
            ]),
        );

        assert_eq!(
            super::parse("{:?}", span).ok(),
            Some(vec![Piece::Debug { pretty: false }]),
        );

        assert_eq!(
            super::parse("{:#?}", span).ok(),
            Some(vec![Piece::Debug { pretty: true }]),
        );

        // escaped braces
        assert_eq!(
            super::parse("{{}} is not an argument", span).ok(),
            Some(vec![Piece::Str(Cow::Borrowed("{} is not an argument"))]),
        );

        // left brace & junk
        assert!(super::parse("{", span).is_err());
        assert!(super::parse(" {", span).is_err());
        assert!(super::parse("{ ", span).is_err());
        assert!(super::parse("{ {", span).is_err());
        assert!(super::parse("{:x}", span).is_err());
    }

    #[test]
    fn unescape() {
        let span = Span::call_site();

        // no right brace
        assert_eq!(super::unescape("", span).ok(), Some(Cow::Borrowed("")));
        assert_eq!(
            super::unescape("Hello", span).ok(),
            Some(Cow::Borrowed("Hello"))
        );

        // unmatched right brace
        assert!(super::unescape(" }", span).is_err());
        assert!(super::unescape("} ", span).is_err());
        assert!(super::unescape("}", span).is_err());

        // escaped right brace
        assert_eq!(super::unescape("}}", span).ok(), Some(Cow::Borrowed("}")));
        assert_eq!(super::unescape("}} ", span).ok(), Some(Cow::Borrowed("} ")));
    }
}
