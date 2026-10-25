//! Procedural macros for Scafra.
//!
//! The macros in this crate intentionally generate small, inspectable impls.
//! They do not perform runtime type discovery.

use proc_macro::TokenStream;

mod bean;
mod config;
mod controller;
mod diagnostics;
mod logger;
#[path = "main.rs"]
mod main_macro;
mod parse;
mod routes;
mod service;

#[proc_macro_attribute]
pub fn service(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(service::expand_service(attr, item))
}

#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(service::expand_service(attr, item))
}

#[proc_macro_attribute]
pub fn repository(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(service::expand_service(attr, item))
}

#[proc_macro_attribute]
pub fn bean(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(bean::expand_bean(attr, item))
}

#[proc_macro_attribute]
pub fn post_processor(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(bean::expand_post_processor(attr, item))
}

#[proc_macro_derive(Config, attributes(config))]
pub fn config(input: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(config::expand_config(input))
}

#[proc_macro_attribute]
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(controller::expand_controller(attr, item))
}

#[proc_macro_attribute]
pub fn routes(_attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(routes::expand_routes(_attr, item))
}

/// Starts a generated Scafra application.
///
/// `config_loader = path::to::function` selects a synchronous, zero-argument
/// function returning `scafra::ConfigLoader`. It runs once after the async main
/// body, before the framework loads and validates configuration.
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(main_macro::expand_main(attr.into(), item.into()))
}

/// Adds structured logging spans to functions and impl methods.
#[proc_macro_attribute]
pub fn logger(_attr: TokenStream, item: TokenStream) -> TokenStream {
    diagnostics::into_token_stream(logger::expand_logger(item.into()))
}

// These are standalone no-op attributes. The routes macro consumes them when
// attached to an impl; keeping the exports gives normal Rust behavior when a
// route attribute is used outside that macro.
#[proc_macro_attribute]
pub fn get(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn post(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn put(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn delete(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[cfg(test)]
mod tests {
    use super::main_macro::expand_main_item;
    use syn::ItemFn;

    #[test]
    fn main_expansion_uses_bounded_startup_logging() {
        let input: ItemFn = syn::parse_quote! {
            async fn main() {}
        };
        let expanded = expand_main_item(input)
            .expect("valid main should expand")
            .to_string();

        assert!(expanded.contains("log_startup_failure"));
        assert!(expanded.contains("run_with_loader_and_resources"));
        assert!(expanded.contains(":: scafra :: ConfigLoader :: new ()"));
        assert!(expanded.contains("__scafra_graph :: compose_async"));
        assert!(!expanded.contains("% error"));
        assert!(!expanded.contains("Scafra application failed"));
    }

    #[test]
    fn main_expansion_keeps_graph_resources_until_shutdown() {
        let input: ItemFn = syn::parse_quote! {
            async fn main() {}
        };
        let expanded = expand_main_item(input)
            .expect("valid main should expand")
            .to_string();
        assert!(expanded.contains("build_finalized_router_with_options_and_resources"));
        assert!(expanded.contains("AsyncResourceSet :: shutdown"));
        assert!(expanded.contains("StartupFailure"));
    }

    #[test]
    fn main_loader_hook_is_called_once_after_the_user_body() {
        let expanded = super::main_macro::expand_main(
            quote::quote!(config_loader = crate::configured_loader,),
            quote::quote!(
                async fn main() {
                    user_initialization();
                }
            ),
        )
        .expect("a function path should expand")
        .to_string();
        assert_eq!(expanded.matches("crate :: configured_loader ()").count(), 1);
        assert!(
            expanded.find("user_initialization ()").unwrap()
                < expanded.find("crate :: configured_loader ()").unwrap()
        );
        assert!(expanded.contains("run_with_loader_and_resources (crate :: configured_loader ()"));
    }

    #[test]
    fn main_loader_options_reject_unknown_duplicate_and_non_path_values() {
        for (options, diagnostic) in [
            (
                quote::quote!(loader = configure),
                "unknown scafra::main option",
            ),
            (
                quote::quote!(config_loader = a, config_loader = b),
                "duplicate config_loader option",
            ),
            (
                quote::quote!(config_loader = configure()),
                "config_loader requires a function path",
            ),
            (
                quote::quote!(config_loader = "configure"),
                "config_loader requires a function path",
            ),
            (
                quote::quote!(config_loader = || configure()),
                "config_loader requires a function path",
            ),
            (quote::quote!(config_loader), "expected `=`"),
        ] {
            let error = super::main_macro::expand_main(
                options,
                quote::quote!(
                    async fn main() {}
                ),
            )
            .expect_err("invalid options must be rejected");
            assert!(error.to_string().contains(diagnostic), "{error}");
        }
    }
}
