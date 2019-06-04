#![recursion_limit="128"]

extern crate proc_macro;
use proc_macro::TokenStream;
use syn::{self, AttributeArgs, ItemStruct, Ident, spanned::Spanned};
use syn::parse::Parser;
use quote::quote;

/// Refer to the documentation of the vptr crate
#[proc_macro_attribute]
pub fn vptr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as AttributeArgs);
    let item = syn::parse_macro_input!(item as ItemStruct);
    match vptr_impl(attr, item) {
        Ok(x) => x,
        Err(e) => e.to_compile_error().into()
    }
}


fn vptr_impl(attr: AttributeArgs, item : ItemStruct) -> Result<TokenStream, syn::Error> {
    let ItemStruct{ attrs , vis, struct_token, ident, generics, fields, semi_token } = item;

    let attr = attr.iter().map(|a| {
        if let syn::NestedMeta::Meta(syn::Meta::Word(i)) = a { Ok(i) } else {
            Err(syn::Error::new(a.span(), "attribute of vptr must be a trait"))
        }
    }).collect::<Result<Vec<_> , _>>()?;

    if let Some(tp) = generics.type_params().next() {
        return Err(syn::Error::new(tp.span(), "vptr does not support generics"));
    }


    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (fields, attr_with_names) = if let syn::Fields::Named(mut n) = fields {
        let attr_with_names : Vec<_> = attr.iter().map(|t| {
            let field_name : Ident = syn::parse_str(&format!("vptr_{}", t))?;
            Ok((t, quote!{ #field_name } ))
        }).collect::<Result<Vec<_>, syn::Error>>()?;
        let parser = syn::Field::parse_named;
        for (trait_, field_name) in &attr_with_names {
            n.named.push(
                parser.parse(quote!(#field_name : vptr::VPtr<#ident #ty_generics, #trait_>).into())?);
        }
        (syn::Fields::Named(n), attr_with_names)
    } else {
        let mut n = if let syn::Fields::Unnamed(n) = fields { n } else {
            syn::FieldsUnnamed{ paren_token: Default::default(), unnamed: Default::default() }
        };
        let count = n.unnamed.len();
        let parser = syn::Field::parse_unnamed;
        for trait_ in &attr {
            n.unnamed.push(
                parser.parse(quote!(vptr::VPtr<#ident #ty_generics, #trait_>).into())?);
        }
        let attr_with_names : Vec<_> = attr.iter().enumerate()
            .map(|(i, t)| {
                let field_name = i + count;
                (t, quote!{ #field_name })
            }).collect();
        (syn::Fields::Unnamed(n), attr_with_names)
    };

    let mut result = quote!(
        #(#attrs)* #vis #struct_token #ident #generics  #fields  #semi_token
    );

    for (trait_, field_name) in attr_with_names {
        result = quote!(#result
            unsafe impl #impl_generics vptr::HasVPtr<#trait_> for #ident #ty_generics #where_clause {
                fn init() -> &'static VTableData {
                    use vptr::internal::{TransmuterTO, TransmuterPtr};
                    static VTABLE : VTableData = VTableData{
                        offset: unsafe {
                            let x: &'static #ident  = TransmuterPtr { int: 0 }.ptr;
                            TransmuterPtr { ptr: &x.#field_name }.int
                        },
                        vtable: unsafe {
                            let x: &'static #ident  = TransmuterPtr::<#ident> { int: 0 }.ptr;
                            TransmuterTO::<#trait_>{ ptr: x }.to.vtable
                        }
                    };
                    &VTABLE
                }

                fn get_vptr(&self) -> &VPtr<Self, #trait_> { &self.#field_name }
            }
        );
    }
    //println!("{}", result.to_string());
    Ok(result.into())
}
