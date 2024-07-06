extern crate proc_macro;


#[derive(deluxe::ExtractAttributes)]
#[deluxe(attributes(sda))]
struct StorageDeriveAttibutes {
	// The identifier used to identify this storage
	#[deluxe(default = None)]
	id: Option<String>,
	#[deluxe(default = false)]
	renderdata: bool,
	// Whether or not this storage should expose serde functions
	// Requires serde::serialize and serde::deserialize
	#[deluxe(default = false)]
	serde: bool,
	#[deluxe(default = false)]
	commands: bool,
	#[deluxe(default = false)]
	lua: bool,
}


fn storage_derive_macro2(input: proc_macro2::TokenStream, component: bool) -> deluxe::Result<proc_macro2::TokenStream> {
	let mut ast: syn::DeriveInput = syn::parse2(input)?;

	let attributes: StorageDeriveAttibutes = deluxe::extract_attributes(&mut ast)?;

	let ident = &ast.ident;
	let storage_id = attributes.id.unwrap_or(ast.ident.clone().to_string());

	let mut other_traits = Vec::new();
	if !attributes.renderdata {
		other_traits.push(quote::quote! {
			impl StorageRenderData for #ident {}
		});
	}
	if !attributes.serde {
		other_traits.push(quote::quote! {
			impl StorageSerde for #ident {}
		});
	}
	if !attributes.commands {
		other_traits.push(quote::quote! {
			impl StorageCommandExpose for #ident {}
		});
	}
	if !attributes.lua {
		other_traits.push(quote::quote! {
			impl StorageLuaExpose for #ident {}
		});
	} else {
		other_traits.push(quote::quote! {
			impl StorageLuaExpose for #ident {
				fn create_scoped_ref<'lua, 'scope>(&'scope self, scope: &mlua::Scope<'lua, 'scope>) -> Option<Result<mlua::AnyUserData<'lua>, mlua::Error>> {
					Some(scope.create_any_userdata_ref(self))
				}
			}
		});
	}

	// Implement either component or resource
	let idk = component
		.then(|| quote::quote! { Component })
		.unwrap_or(quote::quote! { Resource });

	Ok(quote::quote! {
		#( #other_traits )*
		impl Storage for #ident {
			const STORAGE_ID: &'static str = #storage_id;
		}
		impl #idk for #ident {}
	})
}


#[proc_macro_derive(Component, attributes(sda))]
pub fn component_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	storage_derive_macro2(input.into(), true).unwrap().into()
}


#[proc_macro_derive(Resource, attributes(sda))]
pub fn resource_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	storage_derive_macro2(input.into(), false).unwrap().into()
}
