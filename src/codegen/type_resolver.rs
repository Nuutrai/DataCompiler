use crate::{
    ast::parser::Type,
    error::{ErrorKind, ErrorReporter},
};
use std::collections::HashMap;
use std::fmt::format;
use crate::codegen::scope::StructDef;

pub struct TypeResolver {
    aliases: HashMap<String, StructDef>,

}

impl TypeResolver {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: String, ty: StructDef) {
        self.aliases.insert(name, ty);
    }

    pub fn resolve(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::Named(name) => match self.aliases.get(name) {
                Some(_) => Some(Type::Named(name.clone())),
                None => None,
            },
            Type::Data(bits) => Some(Type::Data(*bits)),
            Type::DataArray(bits, n) => Some(Type::DataArray(*bits, *n)),
            Type::Void => Some(Type::Void),
            Type::Ref(inner) => Some(Type::Ref(Box::new(self.resolve(inner)?))),
            Type::Generic(n, ps) => Some(Type::Generic(n.clone(), ps.clone())),
            Type::Pointer(inner) => Some(Type::Pointer(Box::new(self.resolve(inner)?))),
        }
    }

    pub fn llvm_type(&self, ty: &Type) -> Option<String> {
        match self.resolve(ty)? {
            Type::Data(bits) => Some(format!("i{}", bits).to_string()),
            Type::DataArray(bits, 0) => Some("ptr".to_string()),
            Type::DataArray(bits, n) => Some(format!("[{} x i{}]", n, bits)),
            Type::Void => Some("void".to_string()),
            Type::Ref(_) => Some("ptr".to_string()),
            Type::Generic(n, _) => Some(format!("%struct.{}", n)),
            Type::Named(name) => {
                let mut types = String::from("<{ ");
                let strct = self.aliases.get(&name).unwrap();
                for (_, ty) in &strct.fields {
                    types.push_str(format!("{}, ", self.llvm_type(ty).unwrap()).as_str())
                }
                types = types.strip_suffix(", ")?.to_string();
                types.push_str(" }>");
                Some(types)
            },
            Type::Pointer(_) => Some("ptr".to_string()),
        }
    }
}
