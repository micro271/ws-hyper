use proc_macro::TokenStream;
use syn::{BinOp, Expr, Lit, Pat, Token, parse::Parse};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use mongodb::bson::doc;

struct Get {
    db: Database,
    amount: Amount,
    doc: Document,
    type_own: TypeOwn,
    condition_or: Condition,
}

impl ToTokens for Get {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let Get {
            db: Database(db),
            amount: Amount(amount),
            doc: Document(doc),
            type_own: TypeOwn(type_own),
            condition_or: Condition { left, op, right },
        } = self;

        let find = match amount.to_string().as_str() {
            "one" => quote! { find_one },
            "many" => quote! { find },
            _ => quote! {},
        };

        let tmp =  match op {
                BinOp::And(and_and) => todo!(),
                BinOp::Or(or_or) => todo!(),
                BinOp::BitAnd(and) => todo!(),
                BinOp::BitOr(or) => todo!(),
                BinOp::Eq(eq_eq) => {
                    quote!{ doc!{ #left: #right } }
                },
                BinOp::Lt(lt) => todo!(),
                BinOp::Le(le) => todo!(),
                BinOp::Ne(ne) => todo!(),
                BinOp::Ge(ge) => todo!(),
                BinOp::Gt(gt) => todo!(),
                _ => todo!(),
            };

        tokens.extend(quote! {
            #db.document::<#type_own>.(#doc).#find(#tmp)
        });

        todo!()
    }
}

impl Parse for Get {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let database = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let amount = input.parse()?;
        let _: syn::Token![=>] = input.parse()?;
        let _: syn::Token![.] = input.parse()?;
        let doc = input.parse()?;
        let _: syn::Token![if] = input.parse()?;
        let condition_or = input.parse()?;

        let _: syn::Token![.] = input.parse()?;
        let type_own = input.parse()?;


        Ok(Self {
            db: database, 
            amount,
            doc,
            condition_or,
            type_own,
        })
    }
}

struct Database(syn::Ident);

impl Parse for Database {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse().map(Self)
    }
}

struct Amount(syn::Ident);

impl Parse for Amount {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse().map(Self)
    }
}

struct Condition{
    left: Expr,
    op: BinOp,
    right: Expr,
}

impl Parse for Condition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        _ = input.parse::<syn::Token![if]>()?;
        
        
        Ok(
            Self{
                left: input.parse()?,
                op: input.parse()?,
                right: input.parse()?,
            }
        )
    }
}

struct Response(Expr);

impl Parse for Response {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse().map(Self)
    }
}

struct Pattern(Pat);

impl Parse for Pattern {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.call(syn::Pat::parse_single).map(Self)
    }
}

struct Document(syn::Lit);

impl Parse for Document {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse().map(Self)
    }
}

struct TypeOwn(syn::Type);

impl Parse for TypeOwn {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse().map(Self)
    }
}

#[proc_macro]
pub fn get(input: TokenStream) -> TokenStream {
    todo!()
}