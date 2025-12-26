use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr, Token, parse::Parse, parse::ParseStream};
use std::collections::HashMap;
use heck::ToSnakeCase;

/// Options for the gen! macro
#[allow(dead_code)]
struct GenOptions {
    yaml_path: String,
    client: bool,
    server: bool,
    module: Option<String>,
}

/// Custom parser for macro arguments
struct GenInput {
    yaml_path: LitStr,
    options: Vec<(syn::Ident, syn::Lit)>,
}

impl Parse for GenInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let yaml_path: LitStr = input.parse()?;
        let mut options = Vec::new();
        
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: syn::Lit = input.parse()?;
            options.push((key, value));
        }
        
        Ok(GenInput { yaml_path, options })
    }
}

/// Service definition parsed from YAML
#[derive(Debug, serde::Deserialize)]
struct ServiceDefinition {
    service: String,
    methods: HashMap<String, MethodDefinition>,
}

/// Method definition with request and response fields
#[derive(Debug, serde::Deserialize)]
struct MethodDefinition {
    request: HashMap<String, String>,
    response: HashMap<String, String>,
}

/// Main procedural macro entry point
#[proc_macro]
pub fn rpc_gen(input: TokenStream) -> TokenStream {
    let gen_input = parse_macro_input!(input as GenInput);
    
    // Parse options
    let yaml_path = gen_input.yaml_path.value();
    let mut client = true; // default
    let mut server = true; // default
    let mut module: Option<String> = None;
    
    for (key, value) in gen_input.options {
        match key.to_string().as_str() {
            "client" => {
                if let syn::Lit::Bool(lit_bool) = value {
                    client = lit_bool.value();
                }
            }
            "server" => {
                if let syn::Lit::Bool(lit_bool) = value {
                    server = lit_bool.value();
                }
            }
            "module" => {
                if let syn::Lit::Str(lit_str) = value {
                    module = Some(lit_str.value());
                }
            }
            _ => {}
        }
    }
    
    // Read and parse YAML at compile time
    // Try to construct an absolute path using CARGO_MANIFEST_DIR
    let full_path = if std::path::Path::new(&yaml_path).is_absolute() {
        yaml_path.clone()
    } else {
        // Get the directory of the crate being compiled
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| ".".to_string());
        std::path::Path::new(&manifest_dir)
            .join(&yaml_path)
            .to_string_lossy()
            .to_string()
    };
    
    let yaml_content = match std::fs::read_to_string(&full_path) {
        Ok(content) => content,
        Err(e) => {
            return syn::Error::new_spanned(
                gen_input.yaml_path,
                format!("Failed to read YAML file '{}' (tried '{}'): {}", yaml_path, full_path, e)
            )
            .to_compile_error()
            .into();
        }
    };
    
    let service_def: ServiceDefinition = match serde_yaml::from_str(&yaml_content) {
        Ok(def) => def,
        Err(e) => {
            return syn::Error::new_spanned(
                gen_input.yaml_path,
                format!("Failed to parse YAML: {}", e)
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Determine module name
    let module_name = module.unwrap_or_else(|| service_def.service.to_snake_case());
    let module_ident = syn::Ident::new(&module_name, proc_macro2::Span::call_site());
    
    let options = GenOptions {
        yaml_path,
        client,
        server,
        module: Some(module_name),
    };
    
    // Generate code
    let generated_code = generate_code(&service_def, &options);
    
    // Wrap in module
    let output = quote! {
        pub mod #module_ident {
            #generated_code
        }
    };
    
    output.into()
}

/// Generate all code based on service definition and options
fn generate_code(service_def: &ServiceDefinition, options: &GenOptions) -> proc_macro2::TokenStream {
    let imports = generate_imports();
    let constants = generate_constants(service_def);
    let structs = generate_request_response_structs(service_def);
    let client = if options.client {
        generate_client_code(service_def)
    } else {
        quote! {}
    };
    let server = if options.server {
        generate_server_code(service_def)
    } else {
        quote! {}
    };
    
    quote! {
        #imports
        #constants
        #structs
        #client
        #server
    }
}

/// Generate necessary imports
fn generate_imports() -> proc_macro2::TokenStream {
    quote! {
        use std::sync::Arc;
        use async_trait::async_trait;
        use geerpc::{
            RPCStatus, StatusCode,
            client::{RPCClient, RPCClientBuilder},
            server::RPCServer,
        };
        use serde::{Deserialize, Serialize};
    }
}

/// Generate service and method name constants
fn generate_constants(service_def: &ServiceDefinition) -> proc_macro2::TokenStream {
    let service_name = &service_def.service;
    let service_const = syn::Ident::new("SERVICE_NAME", proc_macro2::Span::call_site());
    
    let method_constants: Vec<_> = service_def.methods.keys().map(|method_name| {
        let const_name = format!("METHOD_{}", method_name.to_snake_case().to_uppercase());
        let const_ident = syn::Ident::new(&const_name, proc_macro2::Span::call_site());
        let method_value = method_name.to_snake_case();
        
        quote! {
            const #const_ident: &str = #method_value;
        }
    }).collect();
    
    quote! {
        const #service_const: &str = #service_name;
        #(#method_constants)*
    }
}

/// Generate request and response structs for all methods
fn generate_request_response_structs(service_def: &ServiceDefinition) -> proc_macro2::TokenStream {
    let structs: Vec<_> = service_def.methods.iter().map(|(method_name, method_def)| {
        let request_name = syn::Ident::new(
            &format!("{}Request", method_name),
            proc_macro2::Span::call_site()
        );
        let response_name = syn::Ident::new(
            &format!("{}Response", method_name),
            proc_macro2::Span::call_site()
        );
        
        // Generate request fields
        let request_fields: Vec<_> = method_def.request.iter().map(|(field_name, field_type)| {
            let field_ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
            let type_tokens = parse_type(field_type);
            quote! {
                pub #field_ident: #type_tokens
            }
        }).collect();
        
        // Generate response fields
        let response_fields: Vec<_> = method_def.response.iter().map(|(field_name, field_type)| {
            let field_ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
            let type_tokens = parse_type(field_type);
            quote! {
                pub #field_ident: #type_tokens
            }
        }).collect();
        
        quote! {
            #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
            pub struct #request_name {
                #(#request_fields),*
            }
            
            #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
            pub struct #response_name {
                #(#response_fields),*
            }
        }
    }).collect();
    
    quote! {
        #(#structs)*
    }
}

/// Generate client code
fn generate_client_code(service_def: &ServiceDefinition) -> proc_macro2::TokenStream {
    let client_name = syn::Ident::new(
        &format!("{}Client", service_def.service),
        proc_macro2::Span::call_site()
    );
    
    // Generate method implementations
    let methods: Vec<_> = service_def.methods.keys().map(|method_name| {
        let method_ident = syn::Ident::new(
            &method_name.to_snake_case(),
            proc_macro2::Span::call_site()
        );
        let const_name = format!("METHOD_{}", method_name.to_snake_case().to_uppercase());
        let const_ident = syn::Ident::new(&const_name, proc_macro2::Span::call_site());
        
        let request_type = syn::Ident::new(
            &format!("{}Request", method_name),
            proc_macro2::Span::call_site()
        );
        let response_type = syn::Ident::new(
            &format!("{}Response", method_name),
            proc_macro2::Span::call_site()
        );
        
        quote! {
            pub async fn #method_ident(&self, request: #request_type) -> Result<#response_type, geerpc::Error> {
                let response = self
                    .inner
                    .call(SERVICE_NAME, #const_ident, geerpc::encode_payload(request)?)
                    .await?;
                
                geerpc::decode_payload::<#response_type>(response)
            }
        }
    }).collect();
    
    quote! {
        pub struct #client_name {
            inner: Arc<RPCClient>,
        }
        
        impl #client_name {
            pub async fn try_new(address: String) -> Result<Self, geerpc::Error> {
                let inner = RPCClientBuilder::new().address(address).build().await?;
                Ok(Self { inner })
            }
            
            #(#methods)*
        }
    }
}

/// Generate server code
fn generate_server_code(service_def: &ServiceDefinition) -> proc_macro2::TokenStream {
    let service_trait_name = syn::Ident::new(
        &format!("{}Service", service_def.service),
        proc_macro2::Span::call_site()
    );
    let server_name = syn::Ident::new(
        &format!("{}Server", service_def.service),
        proc_macro2::Span::call_site()
    );
    
    // Generate trait methods
    let trait_methods: Vec<_> = service_def.methods.keys().map(|method_name| {
        let method_ident = syn::Ident::new(
            &method_name.to_snake_case(),
            proc_macro2::Span::call_site()
        );
        let request_type = syn::Ident::new(
            &format!("{}Request", method_name),
            proc_macro2::Span::call_site()
        );
        let response_type = syn::Ident::new(
            &format!("{}Response", method_name),
            proc_macro2::Span::call_site()
        );
        
        quote! {
            async fn #method_ident(&self, request: #request_type) -> Result<#response_type, geerpc::Error>;
        }
    }).collect();
    
    // Generate handler registrations
    let handler_registrations: Vec<_> = service_def.methods.keys().map(|method_name| {
        let method_ident = syn::Ident::new(
            &method_name.to_snake_case(),
            proc_macro2::Span::call_site()
        );
        let const_name = format!("METHOD_{}", method_name.to_snake_case().to_uppercase());
        let const_ident = syn::Ident::new(&const_name, proc_macro2::Span::call_site());
        
        let request_type = syn::Ident::new(
            &format!("{}Request", method_name),
            proc_macro2::Span::call_site()
        );
        let error_message = format!("Failed to {}", method_name.to_snake_case());
        
        quote! {
            {
                let inner = self.inner.clone();
                server.register_service(
                    SERVICE_NAME.to_string(),
                    #const_ident.to_string(),
                    Box::new(move |payload: Vec<u8>| {
                        let inner = inner.clone();
                        Box::pin(async move {
                            let request = geerpc::decode_payload::<#request_type>(payload)
                                .map_err(|_| RPCStatus {
                                    code: StatusCode::Internal,
                                    message: "Failed to decode request".to_string(),
                                })?;
                            let response = inner.#method_ident(request).await.map_err(|_| RPCStatus {
                                code: StatusCode::Internal,
                                message: #error_message.to_string(),
                            })?;
                            geerpc::encode_payload(response).map_err(|_| RPCStatus {
                                code: StatusCode::Internal,
                                message: "Failed to encode response".to_string(),
                            })
                        })
                    }),
                );
            }
        }
    }).collect();
    
    quote! {
        #[async_trait]
        pub trait #service_trait_name: Send + Sync + 'static {
            #(#trait_methods)*
        }
        
        pub struct #server_name<T: #service_trait_name> {
            inner: Arc<T>,
        }
        
        impl<T: #service_trait_name> #server_name<T> {
            pub fn new(service: T) -> Self {
                Self {
                    inner: Arc::new(service),
                }
            }
            
            pub fn register(&self, server: &mut RPCServer) {
                #(#handler_registrations)*
            }
        }
    }
}

/// Parse a type string into TokenStream
fn parse_type(type_str: &str) -> proc_macro2::TokenStream {
    let type_str = type_str.trim();
    
    // Handle generic types like Vec<T>, Option<T>
    if let Some(inner) = type_str.strip_prefix("Vec<").and_then(|s| s.strip_suffix(">")) {
        let inner_type = parse_type(inner);
        return quote! { Vec<#inner_type> };
    }
    
    if let Some(inner) = type_str.strip_prefix("Option<").and_then(|s| s.strip_suffix(">")) {
        let inner_type = parse_type(inner);
        return quote! { Option<#inner_type> };
    }
    
    // Handle basic types
    let type_ident = syn::Ident::new(type_str, proc_macro2::Span::call_site());
    quote! { #type_ident }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_snake_case_conversion() {
        assert_eq!("Ping".to_snake_case(), "ping");
        assert_eq!("GetUser".to_snake_case(), "get_user");
        assert_eq!("HTTPRequest".to_snake_case(), "http_request");
    }
    
    #[test]
    fn test_parse_type() {
        let tokens = parse_type("String");
        assert_eq!(tokens.to_string(), "String");
        
        let tokens = parse_type("Vec<String>");
        assert_eq!(tokens.to_string(), "Vec < String >");
        
        let tokens = parse_type("Option<u32>");
        assert_eq!(tokens.to_string(), "Option < u32 >");
    }
}

