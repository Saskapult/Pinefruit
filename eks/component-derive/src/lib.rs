extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};


/// Derives component with a name from `#[component_name = "whatever"]`. 
#[proc_macro_derive(ComponentName, attributes(component_name))]
pub fn derive_component_name(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let ident = ast.ident;

	let component_name_attribute = ast.attrs.iter()
		.find(|a| a.path().is_ident("component_name")).unwrap();

	let component_name = match &component_name_attribute.meta {
		syn::Meta::NameValue(
			syn::MetaNameValue {
				value: syn::Expr::Lit(syn::ExprLit {
					lit: syn::Lit::Str(s),
					..
				}), 
				..
			}
		) => s,
		_ => panic!("That's not a name value!"),
	}.value();

	impl_component(&ident, &component_name)
}


/// Component name will be the struct identifier. 
#[proc_macro_derive(ComponentIdent)]
pub fn derive_component_ident(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let ident = &ast.ident;
	let ident_str = ast.ident.clone().to_string();

	impl_component(ident, &ident_str)
}


fn impl_component(ident: &Ident, component_name: &String) -> TokenStream {
	quote! {
		impl Component for #ident {
			const COMPONENT_NAME: &'static str = #component_name;
		}
	}.into()
}


#[proc_macro_derive(ResourceIdent)]
pub fn derive_resource_ident(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let ident = &ast.ident;
	let ident_str = ast.ident.clone().to_string();

	impl_resource(ident, &ident_str)
}


fn impl_resource(ident: &Ident, resource_name: &String) -> TokenStream {
	quote! {
		impl Resource for #ident {
			const RESOURCE_NAME: &'static str = #resource_name;
		}
	}.into()
}


#[proc_macro_derive(Snappable)]
pub fn derive_snappable(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let ident = &ast.ident;

	impl_snappable(ident)
}


fn impl_snappable(ident: &Ident) -> TokenStream {
	quote! {
		impl Snappable<'_> for #ident {}
	}.into()	
}
