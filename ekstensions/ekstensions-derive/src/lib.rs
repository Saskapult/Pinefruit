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

// /// This is not a derive macro, and it should not be used by extensions. 
// /// This function runs setup code for static extensions. 
// #[proc_macro]
// pub fn load_static_extensions(
// 	_: proc_macro::TokenStream, 
// ) -> proc_macro::TokenStream {
// 	let cargo_toml_path = Path::new("Cargo.toml");
// 	let cargo_toml_content = std::fs::read_to_string(cargo_toml_path)
// 		.expect("Failed to read Cargo.toml");
// 	let cargo_toml: toml::Table = toml::from_str(&cargo_toml_content)
// 		.expect("Failed to parse Cargo.toml");

// 	let static_extensions = cargo_toml
// 		.get("features").and_then(|v| v.as_table())
// 		.expect("Cargo.toml is missing features section")
// 		.get("static_extensions").and_then(|v| v.as_array())
// 		.expect("static is missing from features section")
// 		.iter().map(|v| v.as_str()).collect::<Option<Vec<_>>>().unwrap();

// 	let dependencies = cargo_toml.get("dependencies").expect("no dependecies?!").as_table().unwrap();
// 	let static_extension_paths = static_extensions.iter()
// 		.map(|name| dependencies.get(*name).unwrap().get("path").and_then(|g| g.as_str()).unwrap())
// 		.collect::<Vec<_>>();

// 	let static_extension_loads = static_extensions.into_iter().zip(static_extension_paths.into_iter()).map(|(n, p)| {
// 		let g: proc_macro::TokenStream = quote::quote! {
// 			info!("Loading static extension #n");
// 			{
// 				use #n;
// 				#n::#n _load(&mut world)
// 			}
// 		}.into();
// 		g
// 	});

// 	let mut all_loads = proc_macro::TokenStream::new();
// 	all_loads.extend(static_extension_loads);

// 	all_loads
// }
