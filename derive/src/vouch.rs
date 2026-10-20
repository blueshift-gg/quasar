//! What a handler's calls vouch for: `#[instruction(n)]` reads the handler
//! and names, per field of its accounts struct, the calls that check it, so
//! the field's load can leave those checks out (`cpi::Vouches`).
//!
//! A macro sees the handler's tokens, not what runs, so it counts a call only
//! when every successful run makes it and it is the first thing the account
//! is handed to. Anything it cannot tell keeps the check.

use std::collections::{HashMap, HashSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, ExprCall, ExprMethodCall, FnArg, Ident, ItemFn, Local, Macro, Member, Pat, Path, Stmt,
    visit::{self, Visit},
};

/// Macros that cannot end the handler with success.
const FALLIBLE_ONLY: [&str; 8] = [
    "require",
    "emit",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "matches",
];

/// The most `let (a, b) = if .. { (x, y) } else { (y, x) };` a handler is
/// read through: each doubles the paths to follow.
const MOST_FORKS: usize = 3;

/// A function that builds a call, and where its `ENFORCES` is.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Source {
    /// `path::function(..)`: the module `path::function`.
    Function(String),
    /// `Type::cpi_accounts(..)`: the constant of `Type`.
    Accounts(String),
}

/// `field` is argument `argument` of a call built by `source`.
struct Vouch {
    field: String,
    source: Source,
    argument: usize,
    /// The field each argument of the call is, or `""`.
    arguments: Vec<String>,
}

/// `struct Vouched` and its `Vouches`, for the handler's module.
pub fn expand(handler: &ItemFn) -> TokenStream {
    let paths = paths(handler);
    let mut sources: Vec<Source> = Vec::new();
    for vouch in paths.iter().flatten() {
        if !sources.contains(&vouch.source) {
            sources.push(vouch.source.clone());
        }
    }
    let enforces = |source: &Source| {
        let index = sources.iter().position(|known| known == source).expect("collected");
        format_ident!("__ENFORCES_{index}")
    };
    // A field counts when a call vouches for it on every path.
    let fields: Vec<&String> = match paths.first() {
        Some(first) => {
            let mut fields: Vec<&String> = first.iter().map(|vouch| &vouch.field).collect();
            fields.dedup();
            fields
                .into_iter()
                .filter(|field| paths.iter().all(|path| path.iter().any(|v| v.field == **field)))
                .collect()
        }
        None => Vec::new(),
    };
    let entries = fields.iter().map(|field| {
        let alternatives = paths.iter().map(|path| {
            let vouch = path.iter().find(|vouch| vouch.field == **field).expect("filtered");
            let (enforces, argument, arguments) =
                (enforces(&vouch.source), vouch.argument, &vouch.arguments);
            quote! {
                ::quasar_lang::cpi::Call {
                    enforces: #enforces,
                    arguments: &[#(#arguments),*],
                    argument: #argument,
                }
            }
        });
        quote!((#field, &[#(#alternatives),*]))
    });
    let consts = sources.iter().map(|source| {
        let name = enforces(source);
        let ty = quote!(::quasar_lang::cpi::Enforces);
        match source {
            // The type's own constant, or `Unvouched`'s empty one.
            Source::Accounts(path) => {
                let path: Path = syn::parse_str(path).expect("a path");
                quote! {
                    const #name: #ty = {
                        #[allow(unused_imports)]
                        use ::quasar_lang::cpi::Unvouched as _;
                        <#path>::ENFORCES
                    };
                }
            }
            // The module named like the function, or the empty one here: an
            // import by name takes the module too where there is one, and
            // comes before the glob.
            Source::Function(path) => {
                let path: Path = syn::parse_str(path).expect("a path");
                let function = &path.segments.last().expect("a segment").ident;
                quote! {
                    const #name: #ty = {
                        mod fallback {
                            pub mod #function {
                                pub const ENFORCES: ::quasar_lang::cpi::Enforces = &[];
                            }
                        }
                        #[allow(unused_imports)]
                        use fallback::*;
                        #[allow(unused_imports)]
                        use #path;
                        #function::ENFORCES
                    };
                }
            }
        }
    });
    quote! {
        #(#consts)*

        /// What the handler's calls check, so the accounts' loads need not.
        pub struct Vouched;

        impl ::quasar_lang::cpi::Vouches for Vouched {
            const VOUCHES: &'static [(&'static str, &'static [::quasar_lang::cpi::Call])] =
                &[#(#entries),*];
        }
    }
}

/// The vouches on each path through the handler; one path unless it forks.
fn paths(handler: &ItemFn) -> Vec<Vec<Vouch>> {
    let Some(ctx) = handler.sig.inputs.first().and_then(|input| match input {
        FnArg::Typed(arg) => match &*arg.pat {
            Pat::Ident(ident) => Some(ident.ident.to_string()),
            _ => None,
        },
        FnArg::Receiver(_) => None,
    }) else {
        return Vec::new();
    };
    // A name bound twice may not be the account it was first bound to.
    let mut bound = Bound::default();
    bound.visit_block(&handler.block);
    let forks = handler.block.stmts.iter().filter(|stmt| fork(stmt).is_some()).count();
    if forks > MOST_FORKS {
        return Vec::new();
    }
    (0..1usize << forks)
        .map(|choice| Reading::new(&ctx, &bound.counts, choice).read(&handler.block.stmts))
        .collect()
}

/// How often each name is bound in the handler.
#[derive(Default)]
struct Bound {
    counts: HashMap<String, usize>,
}

impl<'ast> Visit<'ast> for Bound {
    fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
        *self.counts.entry(pat.ident.to_string()).or_default() += 1;
        visit::visit_pat_ident(self, pat);
    }
}

/// `let (a, b) = if .. { (x, y) } else { (y, x) };`: the names, and each
/// branch's values.
fn fork(stmt: &Stmt) -> Option<(Vec<&Ident>, [Vec<&Expr>; 2])> {
    let Stmt::Local(Local { pat: Pat::Tuple(names), init: Some(init), .. }) = stmt else {
        return None;
    };
    let names: Vec<&Ident> = names
        .elems
        .iter()
        .map(|pat| match pat {
            Pat::Ident(ident) => Some(&ident.ident),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let Expr::If(branches) = &*init.expr else { return None };
    fn values(block: &syn::Block, count: usize) -> Option<Vec<&Expr>> {
        match block.stmts.as_slice() {
            [Stmt::Expr(Expr::Tuple(tuple), None)] if tuple.elems.len() == count => {
                Some(tuple.elems.iter().collect())
            }
            _ => None,
        }
    }
    let (_, otherwise) = branches.else_branch.as_ref()?;
    let Expr::Block(otherwise) = &**otherwise else { return None };
    let count = names.len();
    let branches = [values(&branches.then_branch, count)?, values(&otherwise.block, count)?];
    Some((names, branches))
}

/// One reading of the handler: which name is which field, and what has
/// happened to each field so far.
struct Reading<'a> {
    ctx: &'a str,
    bound: &'a HashMap<String, usize>,
    /// Which branch each fork takes, a bit per fork.
    choice: usize,
    /// Names for the accounts struct itself.
    accounts: HashSet<String>,
    /// Names for its fields.
    fields: HashMap<String, String>,
    /// Fields already handed to something: a call that could have changed
    /// them, or code the macro cannot follow.
    passed: HashSet<String>,
    /// The whole struct was handed to something.
    escaped: bool,
}

impl<'a> Reading<'a> {
    fn new(ctx: &'a str, bound: &'a HashMap<String, usize>, choice: usize) -> Self {
        let (accounts, fields, passed) = (HashSet::new(), HashMap::new(), HashSet::new());
        Self { ctx, bound, choice, accounts, fields, passed, escaped: false }
    }

    fn read(mut self, stmts: &[Stmt]) -> Vec<Vouch> {
        let mut vouches = Vec::new();
        let mut forks = 0;
        for (index, stmt) in stmts.iter().enumerate() {
            if let Some((names, branches)) = fork(stmt) {
                let values = &branches[self.choice >> forks & 1];
                forks += 1;
                let fields: Vec<_> = values.iter().map(|value| self.field(value)).collect();
                if fields.iter().all(Option::is_some) {
                    for (name, field) in names.into_iter().zip(fields.into_iter().flatten()) {
                        self.name(name, field);
                    }
                    continue;
                }
            }
            if self.names(stmt) {
                continue;
            }
            // Past a statement that can return success, no call is certain.
            let mut returns = Returns(false);
            returns.visit_stmt(stmt);
            if returns.0 {
                break;
            }
            let mut uses = Uses { reading: &self, fields: Vec::new(), escaped: false };
            uses.visit_stmt(stmt);
            let (used, escaped) = (uses.fields, uses.escaped);
            self.escaped |= escaped;
            let last = index + 1 == stmts.len();
            if !self.escaped
                && let Some(arguments) = self.call(stmt, last)
            {
                for vouch in arguments {
                    // Its first hand-off, and its only use in this statement.
                    let once = used.iter().filter(|other| **other == vouch.field).count() == 1;
                    if once && !self.passed.contains(&vouch.field) {
                        vouches.push(vouch);
                    }
                }
            }
            self.passed.extend(used);
        }
        vouches
    }

    /// Records `name` as `field`, unless the handler binds the name again.
    fn name(&mut self, name: &Ident, field: String) {
        if self.bound.get(&name.to_string()) == Some(&1) {
            self.fields.insert(name.to_string(), field);
        }
    }

    /// Whether `stmt` only names the accounts or their fields:
    /// `let Take { taker, .. } = ctx.accounts;`, `let vault = &mut ctx.accounts.vault;`.
    fn names(&mut self, stmt: &Stmt) -> bool {
        let Stmt::Local(Local { pat, init: Some(init), .. }) = stmt else { return false };
        if init.diverge.is_some() {
            return false;
        }
        let value = peel(&init.expr);
        match pat {
            Pat::Struct(fields) if self.is_accounts(value) => {
                let simple = fields.fields.iter().all(|field| {
                    matches!((&field.member, &*field.pat), (Member::Named(_), Pat::Ident(_)))
                });
                if !simple {
                    self.escaped = true;
                    return true;
                }
                for field in &fields.fields {
                    if let (Member::Named(member), Pat::Ident(name)) = (&field.member, &*field.pat)
                    {
                        self.name(&name.ident, member.to_string());
                    }
                }
                true
            }
            Pat::Ident(name) if self.is_accounts(value) => {
                if self.bound.get(&name.ident.to_string()) == Some(&1) {
                    self.accounts.insert(name.ident.to_string());
                } else {
                    self.escaped = true;
                }
                true
            }
            Pat::Ident(name) => match self.field(value) {
                Some(field) => {
                    self.name(&name.ident, field);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    /// Whether `expr` is the accounts struct: `ctx.accounts` or a name for it.
    fn is_accounts(&self, expr: &Expr) -> bool {
        match peel(expr) {
            Expr::Field(field) => {
                matches!(&field.member, Member::Named(member) if member == "accounts")
                    && is_name(&field.base, |name| name == self.ctx)
            }
            other => is_name(other, |name| self.accounts.contains(name)),
        }
    }

    /// The field `expr` is, if it is one: its name, or `accounts.field`.
    fn field(&self, expr: &Expr) -> Option<String> {
        match peel(expr) {
            Expr::Path(path) => {
                path.path.get_ident().and_then(|name| self.fields.get(&name.to_string()).cloned())
            }
            Expr::Field(field) if self.is_accounts(&field.base) => match &field.member {
                Member::Named(member) => Some(member.to_string()),
                Member::Unnamed(_) => None,
            },
            _ => None,
        }
    }

    /// The fields `stmt` passes to a call it invokes, if it is one that every
    /// run reaching it makes: `function(..).invoke()?;` at the top of the handler.
    fn call(&self, stmt: &Stmt, last: bool) -> Option<Vec<Vouch>> {
        let expr = match stmt {
            Stmt::Expr(expr, semi) if semi.is_some() || last => expr,
            Stmt::Local(Local { init: Some(init), .. }) if init.diverge.is_none() => &*init.expr,
            _ => return None,
        };
        let expr = match expr {
            Expr::Try(tried) => &*tried.expr,
            other => other,
        };
        let Expr::MethodCall(ExprMethodCall { receiver, method, .. }) = expr else { return None };
        if method != "invoke" && method != "invoke_signed" {
            return None;
        }
        let Expr::Call(built) = &**receiver else { return None };
        let mut arguments = self.arguments(built);
        // `program::instruction::cpi(program, Accounts::cpi_accounts(..), data)`.
        for argument in &built.args {
            if let Expr::Call(inner) = argument {
                arguments.extend(self.arguments(inner));
            }
        }
        Some(arguments)
    }

    /// The fields among `call`'s arguments, with what vouches for each.
    fn arguments(&self, call: &ExprCall) -> Vec<Vouch> {
        let Expr::Path(function) = &*call.func else { return Vec::new() };
        let Some(source) = source(&function.path) else { return Vec::new() };
        let all: Vec<String> =
            call.args.iter().map(|argument| self.field(argument).unwrap_or_default()).collect();
        let fields = all.iter().enumerate().filter(|(_, field)| !field.is_empty());
        fields
            .map(|(argument, field)| Vouch {
                field: field.clone(),
                source: source.clone(),
                argument,
                arguments: all.clone(),
            })
            .collect()
    }
}

/// Where the function at `path` says what it vouches for, if it can: a
/// function in a module, or a type's `cpi_accounts`.
fn source(path: &Path) -> Option<Source> {
    let mut plain = path.clone();
    for segment in &mut plain.segments {
        segment.arguments = syn::PathArguments::None;
    }
    let lowercase =
        |ident: &Ident| ident.to_string().starts_with(|first: char| !first.is_uppercase());
    let (function, module) = plain
        .segments
        .iter()
        .map(|segment| &segment.ident)
        .collect::<Vec<_>>()
        .split_last()
        .map(|(function, module)| (function.to_string(), module.to_vec()))?;
    if function == "cpi_accounts" && !module.is_empty() {
        let mut ty = plain.clone();
        ty.segments.pop();
        ty.segments.pop_punct();
        return Some(Source::Accounts(relative(&ty, false)));
    }
    module.iter().all(|ident| lowercase(ident)).then(|| Source::Function(relative(&plain, true)))
}

/// `path` as the handler writes it, from the module beside the handler: one
/// level down. A lone name is imported from that module, which sees the
/// handler's.
fn relative(path: &Path, import: bool) -> String {
    let written = quote!(#path).to_string();
    let first = path.segments.first().map(|segment| segment.ident.to_string());
    match first.as_deref() {
        _ if path.leading_colon.is_some() => written,
        Some("crate") => written,
        Some("self") => written.replacen("self", "super", 1),
        Some("super") => format!("super :: {written}"),
        _ if import && path.segments.len() == 1 => format!("self :: {written}"),
        _ => written,
    }
}

/// `expr` without the `&`, `&mut`, `*` and parentheses around it.
fn peel(expr: &Expr) -> &Expr {
    match expr {
        Expr::Reference(reference) => peel(&reference.expr),
        Expr::Paren(paren) => peel(&paren.expr),
        Expr::Group(group) => peel(&group.expr),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => peel(&unary.expr),
        other => other,
    }
}

fn is_name(expr: &Expr, is: impl Fn(&str) -> bool) -> bool {
    matches!(expr, Expr::Path(path) if path.path.get_ident().is_some_and(|name| is(&name.to_string())))
}

/// Whether a statement can end the handler with success before its end.
struct Returns(bool);

impl<'ast> Visit<'ast> for Returns {
    fn visit_expr_return(&mut self, _: &'ast syn::ExprReturn) {
        self.0 = true;
    }

    fn visit_macro(&mut self, invoked: &'ast Macro) {
        let known = invoked.path.segments.last().map(|segment| segment.ident.to_string());
        self.0 |= !known.is_some_and(|name| FALLIBLE_ONLY.contains(&name.as_str()));
    }
}

/// The fields a statement hands to something: every mention that is not the
/// receiver of a method or the base of a field read.
struct Uses<'a, 'b> {
    reading: &'a Reading<'b>,
    fields: Vec<String>,
    escaped: bool,
}

impl Uses<'_, '_> {
    /// Visits `expr`, which is only read if it is a field.
    fn read(&mut self, expr: &Expr) {
        if self.reading.field(expr).is_none() {
            self.visit_expr(expr);
        }
    }
}

impl<'ast> Visit<'ast> for Uses<'_, '_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Some(field) = self.reading.field(expr) {
            self.fields.push(field);
            return;
        }
        match peel(expr) {
            // `ctx.data` is not the accounts.
            Expr::Field(field) if is_name(&field.base, |name| name == self.reading.ctx) => {
                self.escaped |= !matches!(&field.member, Member::Named(member) if member == "data");
            }
            Expr::Field(field) => self.read(&field.base),
            Expr::MethodCall(call) => {
                self.read(&call.receiver);
                for argument in &call.args {
                    self.visit_expr(argument);
                }
            }
            Expr::Path(path) => {
                let name = path.path.get_ident().map(Ident::to_string);
                self.escaped |= name.is_some_and(|name| {
                    name == self.reading.ctx || self.reading.accounts.contains(&name)
                });
            }
            other => visit::visit_expr(self, other),
        }
    }

    /// A macro's tokens are not read as code: any field it names counts.
    fn visit_macro(&mut self, invoked: &'ast Macro) {
        fn names(tokens: TokenStream, out: &mut Vec<String>) {
            for token in tokens {
                match token {
                    proc_macro2::TokenTree::Ident(ident) => out.push(ident.to_string()),
                    proc_macro2::TokenTree::Group(group) => names(group.stream(), out),
                    _ => {}
                }
            }
        }
        let mut mentioned = Vec::new();
        names(invoked.tokens.clone(), &mut mentioned);
        for name in mentioned {
            if name == self.reading.ctx || self.reading.accounts.contains(&name) {
                self.escaped = true;
            } else if let Some(field) = self.reading.fields.get(&name) {
                self.fields.push(field.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fields each path through `handler` has vouched for.
    fn vouched(handler: &str) -> Vec<Vec<String>> {
        let handler: ItemFn = syn::parse_str(handler).expect("a function");
        let paths = paths(&handler);
        paths.into_iter().map(|path| path.into_iter().map(|vouch| vouch.field).collect()).collect()
    }

    const HEAD: &str =
        "fn h(mut ctx: Ctx<A>) -> Result<()> { let A { a, b, auth, p } = &mut ctx.accounts;";

    fn body(rest: &str) -> Vec<Vec<String>> {
        vouched(&format!("{HEAD} {rest} }}"))
    }

    // Every field a call takes is listed; its `ENFORCES` says which it vouches for.
    #[test]
    fn a_call_every_run_makes_vouches_for_its_accounts() {
        let all = [["p", "a", "b", "auth"]];
        assert_eq!(body("token::transfer(p, a, b, auth, 1).invoke()?; Ok(())"), all);
        assert_eq!(body("let n = a.amount(); token::transfer(p, a, b, auth, n).invoke()"), all);
        assert_eq!(
            vouched(
                "fn h(ctx: Ctx<A>) -> Result<()> { t::f(&ctx.accounts.p, &mut ctx.accounts.a).invoke() }"
            ),
            [["p", "a"]]
        );
        assert_eq!(body("x::go::cpi(p, X::cpi_accounts(a, b), ()).invoke()"), [["p", "a", "b"]]);
    }

    #[test]
    fn a_call_some_run_may_skip_vouches_for_nothing() {
        let none: [[&str; 0]; 1] = [[]];
        assert_eq!(
            body("if x { return Ok(()); } token::transfer(p, a, b, auth, 1).invoke()"),
            none
        );
        assert_eq!(body("if x { token::transfer(p, a, b, auth, 1).invoke()?; } Ok(())"), none);
        assert_eq!(body("mine!(); token::transfer(p, a, b, auth, 1).invoke()"), none);
        assert_eq!(body("helper(ctx); token::transfer(p, a, b, auth, 1).invoke()"), none);
    }

    #[test]
    fn an_account_handed_on_or_renamed_is_not_vouched_for() {
        let not_a = [["p", "b", "auth"]];
        assert_eq!(body("helper(a); token::transfer(p, a, b, auth, 1).invoke()"), not_a);
        assert_eq!(body("require!(ok(a), E); token::transfer(p, a, b, auth, 1).invoke()"), not_a);
        assert_eq!(body("let a = other; token::transfer(p, a, b, auth, 1).invoke()"), not_a);
        assert_eq!(body("token::transfer(p, a, b, auth, n(b)).invoke()"), [["p", "a", "auth"]]);
    }

    #[test]
    fn a_fork_is_read_down_both_branches() {
        let swap = "let (x, y) = if d { (a, b) } else { (b, a) }; token::transfer(p, x, y, auth, 1).invoke()";
        assert_eq!(body(swap), [["p", "a", "b", "auth"], ["p", "b", "a", "auth"]]);
    }
}
