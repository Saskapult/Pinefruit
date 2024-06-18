use std::path::Path;

extern crate proc_macro;


/// The info fucntion for an extension. 
#[proc_macro_attribute]
pub fn info(_attr: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	rename_fn_to(input, &*format!("{}_info", std::env::var("CARGO_PKG_NAME").unwrap()))
}

/// The load function for an extension. 
/// Please elaborate on what should be done here. 
#[proc_macro_attribute]
pub fn load(_attr: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	rename_fn_to(input, &*format!("{}_load", std::env::var("CARGO_PKG_NAME").unwrap()))
}

/// The systems function for an extension. 
/// Declares the provided systems. 
#[proc_macro_attribute]
pub fn systems(_attr: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	rename_fn_to(input, &*format!("{}_systems", std::env::var("CARGO_PKG_NAME").unwrap()))
}

// Renames a fucntion 
// Also adds no_mangle
// https://users.rust-lang.org/t/using-macros-to-modify-ast-to-modify-and-add-line-of-codes-in-function/56805/5
fn rename_fn_to(input: proc_macro::TokenStream, to: &str) -> proc_macro::TokenStream {
	let ident = syn::Ident::new(to, proc_macro2::Span::call_site());

	let mut item = syn::parse::<syn::Item>(input).unwrap();

	match &mut item {
		syn::Item::Fn(fn_item) => {
			fn_item.sig.ident = ident;
			// println!("{:#?}", fn_item.attrs);
		},
		_ => panic!("non-function passed!"),
	}

	use quote::ToTokens;
	let ts = item.into_token_stream();
	
	let pound = proc_macro2::Punct::new('#', proc_macro2::Spacing::Alone);
	quote::quote! {
		#pound [no_mangle]
		#ts
	}.into()
}

/// This is not a derive macro, and it should not be used by extensions. 
/// Looks for core extensions (those used by the main crate) and loads them. 
/// Outputs an exclude list of core extension paths. 
#[proc_macro]
pub fn load_core_extensions(
	_: proc_macro::TokenStream, 
) -> proc_macro::TokenStream {
	let extension_directory = Path::new("extensions");

	let cargo_toml_path = Path::new("Cargo.toml");
	let cargo_toml_content = std::fs::read_to_string(cargo_toml_path)
		.expect("Failed to read Cargo.toml");
	let cargo_toml: toml::Table = toml::from_str(&cargo_toml_content)
		.expect("Failed to parse Cargo.toml");

	let (
		core_extension_names, 
		core_extension_paths,
	): (Vec<_>, Vec<_>) = cargo_toml
		.get("dependencies").and_then(|v| v.as_table())
		.expect("Cargo.toml is missing dependencies table")
		// Core extensions have "path = '{extensions_directory}/blah'"
		.iter().filter_map(|(name, v)| {
			let path = Path::new(v.as_table()?.get("path")?.as_str()?);
			let parent = Path::new(path.parent()?.file_name()?
				.to_str().expect("No string?"));
			(extension_directory == parent).then_some((name, path))
		})
		.unzip();

	let core_extension_loads = core_extension_names.into_iter().map(|n| {
		let crate_name = quote::format_ident!("{}", n);
		let load_function_name = quote::format_ident!("{}_load", n);
		let systems_function_name = quote::format_ident!("{}_systems", n);
		quote::quote! {
			debug!("Loading core extension {}", #n);
			{
				use #crate_name;
				#crate_name::#load_function_name(&mut esl);
				#crate_name::#systems_function_name(&mut ess);
			}
		}
	}).collect::<Vec<_>>();

	let exclude_list = core_extension_paths.into_iter().map(|p| {
		let s = p.to_str().expect("unexpected");
		quote::quote! {
			Path::new(#s)
		}
	}).collect::<Vec<_>>();

	quote::quote! {		
		{
			#( #core_extension_loads )*
			use std::path::Path;
			vec![#( #exclude_list ),*]
		}
	}.into()
}
