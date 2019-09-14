/* Copyright (C) 2019 Olivier Goffart <ogoffart@woboq.com>

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

#![recursion_limit = "128"]
//! Refer to the documentation of the `vptr` crate

extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{self, spanned::Spanned, AttributeArgs, ItemStruct};

/// Refer to the documentation of the `vptr` crate
#[proc_macro_attribute]
pub fn vptr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as AttributeArgs);
    let item = syn::parse_macro_input!(item as ItemStruct);
    match vptr_impl(attr, item) {
        Ok(x) => x,
        Err(e) => e.to_compile_error().into(),
    }
}

fn vptr_impl(attr: AttributeArgs, item: ItemStruct) -> Result<TokenStream, syn::Error> {
    let ItemStruct {
        attrs,
        vis,
        struct_token,
        ident,
        generics,
        fields,
        semi_token,
    } = item;

    let attr = attr
        .iter()
        .map(|a| {
            if let syn::NestedMeta::Meta(syn::Meta::Path(i)) = a {
                Ok(i.clone())
            } else if let syn::NestedMeta::Lit(syn::Lit::Str(lit_str)) = a {
                lit_str.parse::<syn::Path>()
            } else {
                Err(syn::Error::new(
                    a.span(),
                    "attribute of vptr must be a trait",
                ))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(tp) = generics.type_params().next() {
        return Err(syn::Error::new(tp.span(), "vptr does not support generics"));
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (fields, attr_with_names) = if let syn::Fields::Named(mut n) = fields {
        let attr_with_names: Vec<_> = attr
            .iter()
            .map(|t| {
                let field_name = quote::format_ident!("vptr_{}", t.segments.last().unwrap().ident);
                (t, quote! { #field_name })
            })
            .collect();
        let parser = syn::Field::parse_named;
        for (trait_, field_name) in &attr_with_names {
            n.named
                .push(parser.parse(
                    quote!(#field_name : vptr::VPtr<#ident #ty_generics, dyn #trait_>).into(),
                )?);
        }
        (syn::Fields::Named(n), attr_with_names)
    } else {
        let mut n = if let syn::Fields::Unnamed(n) = fields {
            n
        } else {
            syn::FieldsUnnamed {
                paren_token: Default::default(),
                unnamed: Default::default(),
            }
        };
        let count = n.unnamed.len();
        let parser = syn::Field::parse_unnamed;
        for trait_ in &attr {
            n.unnamed
                .push(parser.parse(quote!(vptr::VPtr<#ident #ty_generics, dyn #trait_>).into())?);
        }
        let attr_with_names: Vec<_> = attr
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let field_name = syn::Index::from(i + count);
                (t, quote! { #field_name })
            })
            .collect();
        (syn::Fields::Unnamed(n), attr_with_names)
    };

    let mut result = quote!(
        #(#attrs)* #[allow(non_snake_case)] #vis #struct_token #ident #generics  #fields  #semi_token
    );

    for (trait_, field_name) in attr_with_names {
        result = quote!(#result
            unsafe impl #impl_generics vptr::HasVPtr<dyn #trait_> for #ident #ty_generics #where_clause {
                fn init() -> &'static VTableData {
                    use vptr::internal::{TransmuterTO, TransmuterPtr};
                    static VTABLE : VTableData = VTableData{
                        offset: unsafe {
                            let x: &'static #ident  = TransmuterPtr { int: 0 }.ptr;
                            TransmuterPtr { ptr: &x.#field_name }.int
                        },
                        vtable: unsafe {
                            let x: &'static #ident  = TransmuterPtr::<#ident> { int: 0 }.ptr;
                            TransmuterTO::<dyn #trait_>{ ptr: x }.to.vtable
                        }
                    };
                    &VTABLE
                }

                fn get_vptr(&self) -> &VPtr<Self, dyn #trait_> { &self.#field_name }
                fn get_vptr_mut(&mut self) -> &mut VPtr<Self, dyn #trait_> { &mut self.#field_name }
            }
        );
    }
    //println!("{}", result.to_string());
    Ok(result.into())
}
