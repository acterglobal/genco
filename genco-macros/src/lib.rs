#![recursion_limit = "256"]

extern crate proc_macro;

use proc_macro2::Span;
use syn::parse::{ParseStream, Parser as _};

mod cursor;
mod encoder;
mod item_buffer;
mod quote_in_parser;
mod quote_parser;

pub(crate) use self::cursor::Cursor;
pub(crate) use self::encoder::{Control, Delimiter, Encoder, MatchArm};
pub(crate) use self::item_buffer::ItemBuffer;

/// Language neutral whitespace sensitive quasi-quoting.
///
/// # Interpolation
///
/// Elements are interpolated using `#`, so to include the variable `test`,
/// you could write `#test`. Returned elements must implement [FormatTokens].
///
/// **Note:** `#` can be escaped by repeating it twice. So `##` would produce a
/// single `#` token.
///
/// ```rust
/// use genco::prelude::*;
///
/// let field_ty = rust::imported("std::collections", "HashMap")
///     .with_arguments((rust::U32, rust::U32));
///
/// let tokens: rust::Tokens = quote! {
///     struct Quoted {
///         field: #field_ty,
///     }
/// };
///
/// assert_eq!(
///     vec![
///         "use std::collections::HashMap;",
///         "",
///         "struct Quoted {",
///         "    field: HashMap<u32, u32>,",
///         "}",
///     ],
///     tokens.to_file_vec().unwrap(),
/// );
/// ```
///
/// <br>
///
/// Inline code can be evaluated through `#(<expr>)`.
///
/// ```rust
/// use genco::prelude::*;
///
/// let world = "world";
///
/// let tokens: genco::Tokens = quote!(hello #(world.to_uppercase()));
///
/// assert_eq!("hello WORLD", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// Interpolations are evaluated in the same scope as where the macro is
/// invoked, so you can make use of keywords like `?` (try) when appropriate.
///
/// ```rust
/// use std::error::Error;
///
/// use genco::prelude::*;
///
/// fn age_fn(age: &str) -> Result<rust::Tokens, Box<dyn Error>> {
///     Ok(quote! {
///         fn age() {
///             println!("You are {} years old!", #(str::parse::<u32>(age)?));
///         }
///     })
/// }
/// ```
///
/// [FormatTokens]: https://docs.rs/genco/0/genco/trait.FormatTokens.html
///
/// # Escape Sequences
///
/// Because this macro is _whitespace sensitive_, it might sometimes be
/// necessary to provide hints of where they should be inserted.
///
/// `quote!` trims any trailing and leading whitespace that it sees. So
/// `quote!(Hello )` is the same as `quote!(Hello)`. To include a space at the
/// end, we can use the special `#<space>` escape sequence: `quote!(Hello#<space>)`.
///
/// The available escape sequences are:
///
/// * `#<space>` — Inserts a space between tokens. This corresponds to the
///   [Tokens::space] function.
///
/// * `#<push>` — Inserts a push operation. Push operations makes sure that
///   any following tokens are on their own dedicated line. This corresponds to
///   the [Tokens::push] function.
///
/// * `#<line>` — Inserts a forced line. Line operations makes sure that
///   any following tokens have an empty line separating them. This corresponds
///   to the [Tokens::line] function.
///
/// ```rust
/// use genco::prelude::*;
///
/// let numbers = 3..=5;
///
/// let tokens: Tokens<()> = quote!(foo#<push>bar#<line>baz#<space>biz);
///
/// assert_eq!("foo\nbar\n\nbaz biz", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// [Tokens::space]: https://docs.rs/genco/0/genco/struct.Tokens.html#method.space
/// [Tokens::push]: https://docs.rs/genco/0/genco/struct.Tokens.html#method.push
/// [Tokens::line]: https://docs.rs/genco/0/genco/struct.Tokens.html#method.line
///
/// # Loops
///
/// To repeat a pattern you can use `#(for <bindings> in <expr> { <quoted> })`,
/// where <expr> is an iterator.
///
/// It is also possible to use the more compact
/// `#(for <bindings> in <expr> => <quoted>)` (note the arrow).
///
/// `<quoted>` will be treated as a quoted expression, so anything which works
/// during regular quoting will work here as well, with the addition that
/// anything defined in `<bindings>` will be made available to the statement.
///
/// ```rust
/// use genco::prelude::*;
///
/// let numbers = 3..=5;
///
/// let tokens: Tokens<()> = quote! {
///     Your numbers are: #(for n in numbers => #n#<space>)
/// };
///
/// assert_eq!("Your numbers are: 3 4 5", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// # Joining Loops
///
/// You can add `join (<quoted>)` to the end of a repitition specification.
///
/// The expression specified in `join (<quoted>)` is added _between_ each
/// element produced by the loop.
///
/// **Note:** The argument to `join` us *whitespace sensitive*, so leading and
/// trailing is preserved. `join (,)` and `join (, )` would therefore produce
/// different results.
///
/// ```rust
/// use genco::prelude::*;
///
/// let numbers = 3..=5;
///
/// let tokens: Tokens<()> = quote! {
///     Your numbers are: #(for n in numbers join (, ) => #n).
/// };
///
/// assert_eq!("Your numbers are: 3, 4, 5.", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// [quote!]: macro.quote.html
///
/// # Conditionals
///
/// You can specify a conditional with `#(if <condition> => <then>)` where
/// <condition> is an expression evaluating to a `bool`, and `<then>` and
/// `<else>` are quoted expressions.
///
/// It's also possible to specify a condition with an else branch, by using
/// `#(if <condition> { <then> } else { <else> })`. In this instance, `<else>`
/// is also a quoted expression.
///
/// ```rust
/// use genco::prelude::*;
///
/// fn greeting(hello: bool, name: &str) -> Tokens<()> {
///     quote!(Custom Greeting: #(if hello {
///         Hello #name
///     } else {
///         Goodbye #name
///     }))
/// }
///
/// let tokens = greeting(true, "John");
/// assert_eq!("Custom Greeting: Hello John", tokens.to_string().unwrap());
///
/// let tokens = greeting(false, "John");
/// assert_eq!("Custom Greeting: Goodbye John", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// The `<else>` branch is optional, so the following is a valid expression that
/// if `false`, won't result in any tokens:
///
/// ```rust
/// use genco::prelude::*;
///
/// fn greeting(hello: bool, name: &str) -> Tokens<()> {
///     quote!(Custom Greeting:#(if hello {
///         #<space>Hello #name
///     }))
/// }
///
/// let tokens = greeting(true, "John");
/// assert_eq!("Custom Greeting: Hello John", tokens.to_string().unwrap());
///
/// let tokens = greeting(false, "John");
/// assert_eq!("Custom Greeting:", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// # Match Statements
///
/// You can specify a match statement with
/// `#(match <condition> { [<pattern> => <quoted>,]* }`, where <condition> is an
/// evaluated expression that is match against each subsequent <pattern>. If a
/// pattern matches, the arm with the matching `<quoted>` block is evaluated.
///
/// ```rust
/// use genco::prelude::*;
///
/// enum Greeting {
///     Hello,
///     Goodbye,
/// }
///
/// fn greeting(greeting: Greeting, name: &str) -> Tokens<()> {
///     quote!(Custom Greeting: #(match greeting {
///         Greeting::Hello => Hello #name,
///         Greeting::Goodbye => Goodbye #name,
///     }))
/// }
///
/// let tokens = greeting(Greeting::Hello, "John");
/// assert_eq!("Custom Greeting: Hello John", tokens.to_string().unwrap());
///
/// let tokens = greeting(Greeting::Goodbye, "John");
/// assert_eq!("Custom Greeting: Goodbye John", tokens.to_string().unwrap());
/// ```
///
/// <br>
///
/// # Scopes
///
/// You can use `#(<binding> => { <quoted> })` to gain mutable access to the current
/// token stream. This is an alternative to existing control flow operators if
/// you want to execute more complex logic during evaluation.
///
/// For a more compact version, you can also omit the braces by doing
/// `#(<binding> => <quoted>)`.
///
/// Note that this can cause borrowing issues if the underlying stream is
/// already a mutable reference. To work around this you can specify
/// `*<binding>` to cause it to reborrow.
///
/// For more information, see [quote_in!].
///
/// ```rust
/// use genco::prelude::*;
///
/// fn quote_greeting(surname: &str, lastname: Option<&str>) -> rust::Tokens {
///     quote! {
///         Hello #surname#(toks => {
///             if let Some(lastname) = lastname {
///                 toks.space();
///                 toks.append(lastname);
///             }
///         })
///     }
/// }
///
/// assert_eq!("Hello John", quote_greeting("John", None).to_string().unwrap());
/// assert_eq!("Hello John Doe", quote_greeting("John", Some("Doe")).to_string().unwrap());
/// ```
///
/// <br>
///
/// ## Whitespace Detection
///
/// The [quote!] macro has the following rules for dealing with indentation and
/// spacing.
///
/// **Spaces** — Two tokens that are separated, are spaced. Regardless of how
/// many spaces there are between them. This can also be controlled manually by
/// inserting the [`#<space>`] escape in the token stream.
///
/// ```rust
/// use genco::prelude::*;
///
/// let tokens: rust::Tokens = quote! {
///     fn     test()     {
///         println!("Hello... ");
///
///         println!("World!");
///     }
/// };
///
/// assert_eq!(
///     vec![
///         "fn test() {",
///         "    println!(\"Hello... \");",
///         "",
///         "    println!(\"World!\");",
///         "}",
///     ],
///     tokens.to_file_vec().unwrap(),
/// )
/// ```
///
/// <br>
///
/// **Line breaking** — Line breaks are detected by leaving two empty lines
/// between two tokens. This can also be controlled manually by inserting the
/// [`#<line>`] escape in the token stream.
///
/// ```rust
/// use genco::prelude::*;
///
/// let tokens: rust::Tokens = quote! {
///     fn test() {
///         println!("Hello... ");
///
///
///
///         println!("World!");
///     }
/// };
///
/// assert_eq!(
///     vec![
///         "fn test() {",
///         "    println!(\"Hello... \");",
///         "",
///         "    println!(\"World!\");",
///         "}",
///     ],
///     tokens.to_file_vec().unwrap(),
/// )
/// ```
///
/// <br>
///
/// **Indentation** — Indentation is determined on a row-by-row basis. If a
/// column is further in than the one on the preceeding row, it is indented
/// *one level* deeper.
///
/// If a column starts shallower than a previous row, it will be matched against
/// previously known indentation levels.
///
/// All indentations inserted during the macro will be unrolled at the end of
/// it. So any trailing indentations will be matched by unindentations.
///
/// ```rust
/// use genco::prelude::*;
///
/// let tokens: rust::Tokens = quote! {
///     fn test() {
///             println!("Hello... ");
///
///             println!("World!");
///     }
/// };
///
/// assert_eq!(
///     vec![
///         "fn test() {",
///         "    println!(\"Hello... \");",
///         "",
///         "    println!(\"World!\");",
///         "}",
///     ],
///     tokens.to_file_vec().unwrap(),
/// )
/// ```
///
/// A mismatched indentation would result in an error:
///
/// ```rust,compile_fail
/// use genco::prelude::*;
///
/// let tokens: rust::Tokens = quote! {
///     fn test() {
///             println!("Hello... ");
///
///         println!("World!");
///     }
/// };
/// ```
///
/// ```text
/// ---- src\lib.rs -  (line 150) stdout ----
/// error: expected 4 less spaces of indentation
/// --> src\lib.rs:157:9
///    |
/// 10 |         println!("World!");
///    |         ^^^^^^^
/// ```
///
/// [`#<space>`]: #escape-sequences
/// [`#<line>`]: #escape-sequences
#[proc_macro]
pub fn quote(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let receiver = &syn::Ident::new("__genco_macros_toks", Span::call_site());

    let parser = quote_parser::QuoteParser::new(receiver);

    let parser = move |stream: ParseStream| parser.parse(stream);

    let output = match parser.parse(input) {
        Ok(data) => data,
        Err(e) => return proc_macro::TokenStream::from(e.to_compile_error()),
    };

    let gen = quote::quote! {{
        let mut #receiver = genco::Tokens::new();

        {
            let mut #receiver = &mut #receiver;
            #output
        }

        #receiver
    }};

    gen.into()
}

/// Same as [quote!], except that it allows for quoting directly to a token
/// stream.
///
/// You specify the destination stream as the first argument, followed by a `=>`
/// and then the code to generate.
///
/// [quote!]: macro.quote.html
///
/// # Example
///
/// ```rust
/// use genco::prelude::*;
///
/// let mut tokens = rust::Tokens::new();
///
/// quote_in! { tokens =>
///     fn foo() -> u32 {
///         42
///     }
/// }
/// ```
///
/// # Example use inside of [quote!]
///
/// [quote_in!] can be used inside of a [quote!] macro by using a scope.
///
/// ```rust
/// use genco::prelude::*;
///
/// let tokens: rust::Tokens = quote! {
///     fn foo(v: bool) -> u32 {
///         #(out => {
///             quote_in! { *out =>
///                 if v {
///                     1
///                 } else {
///                     0
///                 }
///             }
///         })
///     }
/// };
/// ```
///
/// [quote!]: macro.quote.html
/// ```
#[proc_macro]
pub fn quote_in(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let quote_in_parser::QuoteInParser;

    let parser = quote_in_parser::QuoteInParser;

    let parser = move |stream: ParseStream| parser.parse(stream);

    let output = match parser.parse(input) {
        Ok(data) => data,
        Err(e) => return proc_macro::TokenStream::from(e.to_compile_error()),
    };

    let gen = quote::quote! {{
        #output
    }};

    gen.into()
}
