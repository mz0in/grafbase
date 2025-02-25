use darling::ast::Data;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Error, LitInt};

use crate::{
    args::{self, RenameTarget},
    utils::{get_crate_name, get_rustdoc, visible_fn, GeneratorResult},
};

pub fn generate(object_args: &args::MergedObject) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_name(object_args.internal);
    let ident = &object_args.ident;
    let (impl_generics, ty_generics, where_clause) = object_args.generics.split_for_impl();
    let extends = object_args.extends;
    let gql_typename = object_args
        .name
        .clone()
        .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));

    let desc = get_rustdoc(&object_args.attrs)
        .map(|s| quote! { ::std::option::Option::Some(::std::borrow::ToOwned::to_owned(#s)) })
        .unwrap_or_else(|| quote! {::std::option::Option::None});

    let s = match &object_args.data {
        Data::Struct(e) => e,
        _ => return Err(Error::new_spanned(ident, "MergedObject can only be applied to an struct.").into()),
    };

    let mut types = Vec::new();
    for field in &s.fields {
        types.push(&field.ty);
    }

    let create_merged_obj = {
        let mut obj = quote! { #crate_name::MergedObjectTail };
        for i in 0..types.len() {
            let n = LitInt::new(&format!("{i}"), Span::call_site());
            obj = quote! { #crate_name::MergedObject(&self.#n, #obj) };
        }
        quote! {
            #obj
        }
    };

    let merged_type = {
        let mut obj = quote! { #crate_name::MergedObjectTail };
        for ty in &types {
            obj = quote! { #crate_name::MergedObject::<#ty, #obj> };
        }
        obj
    };

    let visible = visible_fn(&object_args.visible);
    let resolve_container = if object_args.serial {
        quote! { #crate_name::resolver_utils::resolve_container_serial_native(ctx, self).await }
    } else {
        quote! { #crate_name::resolver_utils::resolve_container_native(ctx, self).await }
    };

    let expanded = quote! {
        #[allow(clippy::all, clippy::pedantic)]
        #[#crate_name::async_trait::async_trait]
        impl #impl_generics #crate_name::resolver_utils::ContainerType for #ident #ty_generics #where_clause {
            async fn resolve_field(&self, ctx: &#crate_name::ContextField<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::ResponseNodeId>> {
                #create_merged_obj.resolve_field(ctx).await
            }

            async fn find_entity(&self, ctx: &#crate_name::ContextField<'_>, params: &#crate_name::Value) ->  #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
               #create_merged_obj.find_entity(ctx, params).await
            }
        }

        #[allow(clippy::all, clippy::pedantic)]
        #[#crate_name::async_trait::async_trait]
        impl #impl_generics #crate_name::LegacyOutputType for #ident #ty_generics #where_clause {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                ::std::borrow::Cow::Borrowed(#gql_typename)
            }

            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> #crate_name::registry::MetaFieldType {
                registry.create_output_type::<Self, _>(|registry| {
                    let mut fields = ::std::default::Default::default();
                    let mut cache_control = ::std::default::Default::default();

                    if let #crate_name::registry::MetaType::Object(#crate_name::registry::ObjectType {
                        fields: obj_fields,
                        cache_control: obj_cache_control,
                        ..
                    }) = registry.create_fake_output_type::<#merged_type>() {
                        fields = obj_fields;
                        cache_control = obj_cache_control;
                    }

                    #crate_name::registry::MetaType::Object(#crate_name::registry::ObjectType {
                        name: ::std::borrow::ToOwned::to_owned(#gql_typename),
                        description: #desc,
                        fields,
                        cache_control,
                        extends: #extends,
                        visible: #visible,
                        is_subscription: false,
                        rust_typename: ::std::borrow::ToOwned::to_owned(::std::any::type_name::<Self>()),
                        constraints: vec![],
                        external: false
                        shareable: false,
                    })
                })
            }

            async fn resolve(&self, ctx: &#crate_name::ContextSelectionSetLegacy<'_>, _field: &#crate_name::Positioned<#crate_name::parser::types::Field>) -> #crate_name::ServerResult<#crate_name::ResponseNodeId> {
                #resolve_container
            }
        }

        impl #impl_generics #crate_name::ObjectType for #ident #ty_generics #where_clause {}
    };
    Ok(expanded.into())
}
