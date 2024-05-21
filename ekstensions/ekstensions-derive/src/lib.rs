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
			println!("{:#?}", fn_item.attrs);
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
