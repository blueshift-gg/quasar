use syn::Item;

pub fn file_contains_drain_pattern(file: &syn::File) -> bool {
    use syn::visit::Visit;

    const DRAIN_NAMES: &[&str] = &["set_lamports", "sub_lamports", "assign"];

    struct DrainFinder {
        found: bool,
    }

    impl<'ast> Visit<'ast> for DrainFinder {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if self.found {
                return;
            }
            if DRAIN_NAMES.iter().any(|n| node.method == n) {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }

        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if self.found {
                return;
            }
            if let syn::Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    if DRAIN_NAMES.iter().any(|n| seg.ident == n) {
                        self.found = true;
                        return;
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    let mut finder = DrainFinder { found: false };
    finder.visit_file(file);
    finder.found
}

pub fn impl_blocks_for<'a>(
    file: &'a syn::File,
    struct_name: &str,
) -> impl Iterator<Item = &'a syn::ItemImpl> {
    let struct_name = struct_name.to_string();
    file.items.iter().filter_map(move |item| match item {
        Item::Impl(impl_block) => {
            let matches = matches!(
                impl_block.self_ty.as_ref(),
                syn::Type::Path(tp) if tp.path.segments.iter().any(|s| s.ident == struct_name)
            );
            matches.then_some(impl_block)
        }
        _ => None,
    })
}

pub fn impl_block_contains_zero_drain(file: &syn::File, struct_name: &str) -> bool {
    use syn::visit::Visit;

    struct ZeroDrainFinder {
        found: bool,
    }

    fn has_zero_literal(args: &syn::punctuated::Punctuated<syn::Expr, syn::token::Comma>) -> bool {
        args.iter().any(|arg| {
            if let syn::Expr::Lit(lit) = arg {
                if let syn::Lit::Int(int) = &lit.lit {
                    return int.base10_digits() == "0";
                }
            }
            false
        })
    }

    impl<'ast> Visit<'ast> for ZeroDrainFinder {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if self.found {
                return;
            }
            if node.method == "set_lamports" && has_zero_literal(&node.args) {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }

        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if self.found {
                return;
            }
            if let syn::Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    if seg.ident == "set_lamports" && has_zero_literal(&node.args) {
                        self.found = true;
                        return;
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    impl_blocks_for(file, struct_name).any(|impl_block| {
        let mut finder = ZeroDrainFinder { found: false };
        finder.visit_item_impl(impl_block);
        finder.found
    })
}

pub fn impl_block_contains_call(file: &syn::File, struct_name: &str, fn_name: &str) -> bool {
    use syn::visit::Visit;

    struct CallFinder<'a> {
        target: &'a str,
        found: bool,
    }

    impl<'a, 'ast> Visit<'ast> for CallFinder<'a> {
        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if self.found {
                return;
            }
            if let syn::Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    if seg.ident == self.target {
                        self.found = true;
                        return;
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }

        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if self.found {
                return;
            }
            if node.method == self.target {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }

    impl_blocks_for(file, struct_name).any(|impl_block| {
        let mut finder = CallFinder {
            target: fn_name,
            found: false,
        };
        finder.visit_item_impl(impl_block);
        finder.found
    })
}
