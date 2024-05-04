extern crate proc_macro;


#[derive(deluxe::ExtractAttributes)]
#[deluxe(attributes(metadata))]
struct StorageDeriveAttibutes {
	// The identifier used to identify this storage
	#[deluxe(default = None)]
	id: Option<String>,
	// Whether or not to snap this storage
	// Requires serde::serialize and serde::deserializes
	#[deluxe(default = false)]
	snap: bool,
	// Transformation of data for shader input
	// If none then we just take the bytes directly 
	#[deluxe(default = None)]
	render_transform: Option<String>,
}


fn storage_derive_macro2(input: proc_macro2::TokenStream, component: bool) -> deluxe::Result<proc_macro2::TokenStream> {
	let mut ast: syn::DeriveInput = syn::parse2(input)?;

	let attributes: StorageDeriveAttibutes = deluxe::extract_attributes(&mut ast)?;

	let ident = &ast.ident;
	let storage_id = attributes.id.unwrap_or(ast.ident.clone().to_string());

	let serialize_fn = quote::quote! {
		Some(|item: &#ident, buffer: &mut Vec<u8>| bincode::serialize_into(buffer, item))
	};
	let serialize_many_fn = quote::quote! {
		Some(|item: &[#ident]| bincode::serialize(item))
	};
	let deserialize_fn = quote::quote! {
		Some(|data: &[u8]| bincode::deserialize(data))
	};

	let serial_fn = attributes.snap.then(|| quote::quote! {
		Some((
			#serialize_fn,
			#serialize_many_fn,
			#deserialize_fn,
			#deserialize_fn,
		))
	}).unwrap_or_else(|| quote::quote! {
		None
	});

	let render_fn = attributes.render_transform.and_then(|f| Some(quote::quote! {
		Some(#f)
	})).unwrap_or_else(|| quote::quote! {
		None
	});

	// Implement either component or storage
	// It feels bad but I don't really care
	let idk = component
		.then(|| quote::quote! { Component })
		.unwrap_or(quote::quote! { Resource });

	Ok(quote::quote! {
		impl Storage for #ident {
			const STORAGE_ID: &'static str = #storage_id;
			const SERIALIZE_FN: Option<(fn(&Self, &mut Vec<u8>) -> bincode::Result<()>, fn(&[Self]) -> bincode::Result<()>, fn(&[u8]) -> bincode::Result<Self>, fn(&[u8]) -> bincode::Result<Vec<Self>>)> = #serial_fn;
			const RENDERDATA_FN: Option<fn(&Self, &mut Vec<u8>) -> bincode::Result<()>> = #render_fn;
		}
		impl #idk for #ident {}
	})
}


/// - `id` (String) overrides storage ID
/// - `snap` (bool) flags component for snapping 
/// - `render_transform` (fn(&Self, &mut Vec<u8>) -> bincode::Result<()>) transforms component to shader data
#[proc_macro_derive(Component, attributes(storage_options))]
pub fn component_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	storage_derive_macro2(input.into(), true).unwrap().into()
}


#[proc_macro_derive(Resource, attributes(storage_options))]
pub fn resource_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	storage_derive_macro2(input.into(), false).unwrap().into()
}
