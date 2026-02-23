use crate::{
    ast::parser::Type,
    error::{ErrorKind, ErrorReporter},
};
use std::collections::HashMap;

pub struct TypeResolver {
    aliases: HashMap<String, Type>,
}

impl TypeResolver {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: String, ty: Type) {
        self.aliases.insert(name, ty);
    }

    pub fn resolve(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::Named(name) => match self.aliases.get(name) {
                Some(inner) => self.resolve(inner),
                None => None,
            },
            Type::Data => Some(Type::Data),
            Type::DataArray(n) => Some(Type::DataArray(*n)),
        }
    }

    pub fn llvm_type(&self, ty: &Type) -> Option<String> {
        match self.resolve(ty)? {
            Type::Data => Some("i8".to_string()),
            Type::DataArray(0) => Some("ptr".to_string()),
            Type::DataArray(n) => Some(format!("[{} x i8]", n)),
            Type::Named(_) => unreachable!(),
        }
    }
}
