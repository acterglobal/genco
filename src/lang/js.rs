//! Specialization for JavaScript code generation.
//!
//! # Examples
//!
//! Basic example:
//!
//! ```rust
//! #[feature(proc_macro_hygiene)]
//! use genco::prelude::*;
//!
//! let toks: js::Tokens = quote! {
//!     function foo(v) {
//!         return v + ", World";
//!     }
//!
//!     foo("Hello");
//! };
//!
//! assert_eq!(
//!     vec![
//!         "function foo(v) {",
//!         "    return v + \", World\";",
//!         "}",
//!         "",
//!         "foo(\"Hello\");",
//!     ],
//!     toks.to_file_vec().unwrap()
//! );
//! ```
//!
//! String quoting in JavaScript:
//!
//! ```rust
//! #[feature(proc_macro_hygiene)]
//! use genco::prelude::*;
//!
//! let toks: js::Tokens = quote!(#("hello \n world".quoted()));
//! assert_eq!("\"hello \\n world\"", toks.to_string().unwrap());
//! ```

use crate::{Formatter, ItemStr, Lang, LangItem};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

/// Tokens container specialization for Rust.
pub type Tokens = crate::Tokens<JavaScript>;

impl_lang_item!(Type, JavaScript);

static SEP: &'static str = ".";
static PATH_SEP: &'static str = "/";

/// An imported item in JavaScript.
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Type {
    /// Module of the imported name.
    module: Option<ItemStr>,
    /// Name imported.
    name: ItemStr,
    /// Alias of module.
    alias: Option<ItemStr>,
}

impl Type {
    /// Alias the given type.
    pub fn alias<N: Into<ItemStr>>(self, alias: N) -> Self {
        Self {
            alias: Some(alias.into()),
            ..self
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if let Some(alias) = &self.alias {
            fmt.write_str(alias)?;
            fmt.write_str(SEP)?;
        }

        fmt.write_str(self.name.as_ref())?;
        Ok(())
    }
}

impl LangItem<JavaScript> for Type {
    fn format(&self, out: &mut Formatter, _: &mut (), _: usize) -> fmt::Result {
        write!(out, "{}", self)
    }

    fn as_import(&self) -> Option<&Self> {
        Some(self)
    }
}

/// JavaScript language specialization.
pub struct JavaScript(());

impl JavaScript {
    /// Convert a module into a path.
    fn module_to_path(path: &str) -> String {
        let parts: Vec<&str> = path.split(SEP).collect();
        format!("{}.js", parts.join(PATH_SEP))
    }

    /// Translate imports into the necessary tokens.
    fn imports(tokens: &Tokens) -> Option<Tokens> {
        use crate::ext::QuotedExt as _;

        let mut sets = BTreeMap::new();
        let mut wildcard = BTreeSet::new();

        for import in tokens.walk_imports() {
            match (&import.module, &import.alias) {
                (&Some(ref module), &None) => {
                    sets.entry(module.clone())
                        .or_insert_with(Tokens::new)
                        .append(import.name.clone());
                }
                (&Some(ref module), &Some(ref alias)) => {
                    wildcard.insert((module.clone(), alias.clone()));
                }
                _ => {}
            }
        }

        if wildcard.is_empty() {
            return None;
        }

        let mut out = Tokens::new();

        for (module, names) in sets {
            let mut s = Tokens::new();

            s.append("import {");

            let mut it = names.into_iter();

            if let Some(name) = it.next() {
                s.append(name);
            }

            for name in it {
                s.append(", ");
                s.append(name);
            }

            s.append("} from ");
            s.append(Self::module_to_path(&*module).quoted());
            s.append(";");

            out.append(s);
            out.push();
        }

        for (module, alias) in wildcard {
            let mut s = Tokens::new();

            s.append("import * as ");
            s.append(alias);
            s.append(" from ");
            s.append(Self::module_to_path(&*module).quoted());
            s.append(";");

            out.append(s);
            out.push();
        }

        Some(out)
    }
}

impl Lang for JavaScript {
    type Config = ();
    type Import = Type;

    fn quote_string(out: &mut Formatter, input: &str) -> fmt::Result {
        out.write_char('"')?;

        for c in input.chars() {
            match c {
                '\t' => out.write_str("\\t")?,
                '\u{0007}' => out.write_str("\\b")?,
                '\n' => out.write_str("\\n")?,
                '\r' => out.write_str("\\r")?,
                '\u{0014}' => out.write_str("\\f")?,
                '\'' => out.write_str("\\'")?,
                '"' => out.write_str("\\\"")?,
                '\\' => out.write_str("\\\\")?,
                c => out.write_char(c)?,
            };
        }

        out.write_char('"')?;

        Ok(())
    }

    fn write_file(
        tokens: Tokens,
        out: &mut Formatter,
        config: &mut Self::Config,
        level: usize,
    ) -> fmt::Result {
        let mut toks = Tokens::new();

        if let Some(imports) = Self::imports(&tokens) {
            toks.append(imports);
            toks.push_line();
        }

        toks.append(tokens);
        toks.format(out, config, level)
    }
}

/// Setup an imported element.
///
/// # Examples
///
/// ```rust
/// #![feature(proc_macro_hygiene)]
/// use genco::prelude::*;
///
/// let toks = quote! {
///     #(js::imported("collections", "vec"))
///     #(js::imported("collections", "vec").alias("list"))
/// };
///
/// assert_eq!(
///     vec![
///         "import {vec} from \"collections.js\";",
///         "import * as list from \"collections.js\";",
///         "",
///         "vec",
///         "list.vec",
///     ],
///     toks.to_file_vec().unwrap()
/// );
/// ```
pub fn imported<M, N>(module: M, name: N) -> Type
where
    M: Into<ItemStr>,
    N: Into<ItemStr>,
{
    Type {
        module: Some(module.into()),
        name: name.into(),
        alias: None,
    }
}

/// Setup a local element.
///
/// # Examples
///
/// ```rust
/// #![feature(proc_macro_hygiene)]
/// use genco::prelude::*;
///
/// let toks = quote!(#(js::local("MyType")));
/// assert_eq!(vec!["MyType"], toks.to_file_vec().unwrap());
/// ```
pub fn local<N>(name: N) -> Type
where
    N: Into<ItemStr>,
{
    Type {
        module: None,
        name: name.into(),
        alias: None,
    }
}
