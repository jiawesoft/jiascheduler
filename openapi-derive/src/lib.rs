use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ApiStdResponse)]
pub fn derive_api_std_response(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let response_result = Ident::new(&format!("{}Result", name), name.span());
    let response_name = Ident::new(&format!("{}Response", name), name.span());

    let v = quote! {

        #[derive(Object, Default)]
        pub struct #response_name {
            pub code: u32,
            pub status: bool,
            pub message: String,
            pub data:Option<#name>
        }

        #[derive(poem_openapi::ApiResponse)]
        pub enum #response_result {
            /// Returns when the pet is successfully created.
            #[oai(status = 200)]
            Success(poem_openapi::payload::Json<#response_name>),
            #[oai(status = 500)]
            Error(poem_openapi::payload::Json<crate::response::StdResponse>)
        }

    };

    v.into()
}

#[proc_macro]
pub fn return_success(input: TokenStream) -> TokenStream {
    let mut iter = input.into_iter();

    let ty = iter.nth(0).unwrap().to_string();
    let name = iter.nth(1).unwrap().to_string();

    let ret = format!(
        r##"
    {ty}Result::Success(poem_openapi::payload::Json(
        {ty}Response {{
            code: 0000,
            status: true,
            data: Some({name}),
            message: "success".to_string(),
        }},
    ))
    "##
    );

    ret.parse().unwrap()
}
