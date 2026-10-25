#![allow(dead_code)]

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::{collections::BTreeMap, env, fs, path::PathBuf};
use syn::{
    parse::{Parse, ParseStream},
    Error, Expr, ItemFn, LitStr, Path, Result, ReturnType, Token,
};

#[derive(Default)]
struct MainOptions {
    config_loader: Option<Path>,
}

impl Parse for MainOptions {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut options = Self::default();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            if key != "config_loader" {
                return Err(Error::new_spanned(
                    key,
                    "unknown scafra::main option; expected config_loader",
                ));
            }
            if options.config_loader.is_some() {
                return Err(Error::new_spanned(key, "duplicate config_loader option"));
            }
            input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;
            match value {
                Expr::Path(value) if value.qself.is_none() => {
                    options.config_loader = Some(value.path);
                }
                value => {
                    return Err(Error::new_spanned(
                        value,
                        "config_loader requires a function path, such as crate::configured_loader",
                    ))
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(options)
    }
}

#[derive(Default)]
struct ModuleTree {
    files: BTreeMap<String, PathBuf>,
    children: BTreeMap<String, ModuleTree>,
}

pub(crate) fn expand_main(
    attr: TokenStream,
    item: TokenStream,
) -> Result<proc_macro2::TokenStream> {
    let options = syn::parse2::<MainOptions>(attr)?;
    let input = syn::parse2::<ItemFn>(item)?;
    expand_main_with_options(input, options)
}

pub(crate) fn expand_main_item(input: ItemFn) -> Result<proc_macro2::TokenStream> {
    expand_main_with_options(input, MainOptions::default())
}

fn expand_main_with_options(input: ItemFn, options: MainOptions) -> Result<TokenStream> {
    if input.sig.ident != "main" {
        return Err(Error::new_spanned(
            &input.sig.ident,
            "scafra::main must be applied to a function named 'main'",
        ));
    }
    if input.sig.asyncness.is_none() {
        return Err(Error::new_spanned(
            input.sig.fn_token,
            "scafra::main requires an async function",
        ));
    }
    if !input.sig.inputs.is_empty() {
        return Err(Error::new_spanned(
            &input.sig.inputs,
            "scafra::main does not accept function arguments",
        ));
    }
    if !matches!(input.sig.output, ReturnType::Default) {
        return Err(Error::new_spanned(
            &input.sig.output,
            "scafra::main currently requires no explicit return type",
        ));
    }
    let attrs = input.attrs;
    let vis = input.vis;
    let body = input.block;
    let loader = match options.config_loader {
        Some(path) => quote!(#path()),
        None => quote!(::scafra::ConfigLoader::new()),
    };
    let modules = discover_modules()?;
    let graph_source = scafra_build::render_application_graph(
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest was checked"))
            .join("src/main"),
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest was checked")),
    )
    .map_err(|error| Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    let graph = graph_source.parse::<TokenStream>().map_err(|error| {
        Error::new(
            proc_macro2::Span::call_site(),
            format!("could not render Scafra dependency graph: {error}"),
        )
    })?;
    Ok(quote! {
        ::scafra::__scafra_require_web! {
        #modules
        #graph

        #(#attrs)*
        #vis fn main() {
            if cfg!(debug_assertions)
                && ::std::env::var_os("SCAFRA_DEV_CHILD").is_none()
                && ::std::env::var("SCAFRA_DEV_RELOAD").as_deref() != Ok("false")
            {
                let exit_code = match ::std::env::current_exe() {
                    Ok(executable) => ::scafra::__private::run_dev_supervisor(
                        env!("CARGO_MANIFEST_DIR"),
                        executable,
                    ),
                    Err(error) => {
                        eprintln!("Scafra could not locate the application binary: {error}");
                        1
                    }
                };
                ::std::process::exit(exit_code);
            }
            let runtime = match ::scafra::__private::tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("Scafra could not create the Tokio runtime: {error}");
                    ::std::process::exit(1);
                }
            };
            runtime.block_on(async {
                    #body
                    if let Err(error) = ::scafra::run_with_loader_and_resources(#loader, |config| async move {
                        let graph = __scafra_graph::compose_async_with_policy(
                            __scafra_graph::Overrides::default(), config.shutdown.policy(),
                        ).await?;
                        let (router, resources) = graph.build_finalized_router_with_options_and_resources(
                            &::scafra::router_options(&config),
                        );
                        match router {
                            Ok(router) => Ok((router, resources)),
                            Err(error) => {
                                if let Err(cleanup) = ::scafra::core::AsyncResourceSet::shutdown(
                                    &resources,
                                    ::scafra::ShutdownReason::StartupFailure,
                                    config.shutdown.policy(),
                                ).await {
                                    ::scafra::__private::log_cleanup_failure(
                                        &::scafra::StartupError::from(cleanup),
                                    );
                                }
                                Err(::scafra::StartupError::from(error))
                            }
                        }
                    }).await {
                        ::scafra::__private::log_startup_failure(&error);
                        ::std::process::exit(1);
                    }
            });
        }
        }
    })
}

fn discover_modules() -> Result<TokenStream> {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        Error::new(
            proc_macro2::Span::call_site(),
            "CARGO_MANIFEST_DIR is not set",
        )
    })?;
    let root = PathBuf::from(&manifest).join("src/main");
    if !root.exists() {
        return Ok(TokenStream::new());
    }
    let mut tree = ModuleTree::default();
    collect_modules(&root, &mut tree)?;
    render_modules(&tree)
}

fn collect_modules(directory: &std::path::Path, tree: &mut ModuleTree) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| Error::new(proc_macro2::Span::call_site(), error.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') && name != "target" {
                collect_modules(&path, tree.children.entry(name).or_default())?;
            }
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("rs")
        {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if !matches!(stem, "main" | "lib" | "mod") {
                let module = sanitize_ident(stem);
                if tree.files.insert(module, path).is_some() {
                    return Err(Error::new(
                        proc_macro2::Span::call_site(),
                        "duplicate Scafra module name",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn render_modules(tree: &ModuleTree) -> Result<TokenStream> {
    let mut output = TokenStream::new();
    for (module, path) in &tree.files {
        let ident = format_ident!("{module}");
        let path = LitStr::new(&path.display().to_string(), proc_macro2::Span::call_site());
        output.extend(quote! { #[path = #path] pub mod #ident; });
    }
    for (module, child) in &tree.children {
        let ident = format_ident!("{}", sanitize_ident(module));
        let nested = render_modules(child)?;
        output.extend(quote! { pub mod #ident { #nested } });
    }
    Ok(output)
}

fn sanitize_ident(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if result.is_empty() {
        result.push('_');
    }
    if result
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        result.insert(0, '_');
    }
    result
}

#[allow(dead_code)]
fn main() {}
