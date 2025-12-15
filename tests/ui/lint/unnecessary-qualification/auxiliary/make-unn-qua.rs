#![feature(proc_macro_quote, proc_macro_span)]

extern crate proc_macro;

use proc_macro::{Ident, Literal, Span, TokenStream, TokenTree, quote};

// Expects `fn bar() { if true {} }`
#[proc_macro_attribute]
pub fn bad_unn_qua(_: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = input.into_iter();

    let fn_ = input.next().unwrap();
    let ident = input.next().unwrap();
    let parens = input.next().unwrap();

    let TokenTree::Group(body) = input.next().unwrap() else {
        unreachable!();
    };
    let mut input = body.stream().into_iter();

    let if_ = input.next().unwrap();
    let true_ = input.next().unwrap();
    let braces_inner = input.next().unwrap();

    let span = if_.span().start();

    let call: TokenStream = quote! {
        unn_qua_helper::a::b::foo();
    };
    let call_tokens = call.into_iter();
    let call: TokenStream = call_tokens.map(|mut tt| {
        tt.set_span(span);
        tt
    }).collect();

    quote! {
        $fn_ $ident $parens {
            $if_ $true_ $braces_inner

            $call
        }
    }
}
