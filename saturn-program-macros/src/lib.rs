extern crate proc_macro;

mod program;

#[proc_macro_attribute]
pub fn saturn_program(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    program::expand(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn declare_id(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    program::id::declare_id(item.into()).into()
}
