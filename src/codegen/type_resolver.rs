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
            Type::Array(ty, n) => Some(Type::Array(ty.clone(), *n)),
            Type::Void => Some(Type::Void),
            Type::Ref(inner) => Some(Type::Ref(Box::new(self.resolve(inner)?))),
            Type::Generic(name, types) => Some(Type::Generic(name.clone(), types.clone())),
            Type::Pointer(inner) => Some(Type::Pointer(Box::new(self.resolve(inner)?))),
        }
    }

    pub fn llvm_type(&self, ty: &Type) -> Option<String> {
        match self.resolve(ty)? {
            Type::Data(bits) => Some(format!("i{}", bits).to_string()),
            Type::Array(ty, 0) => Some("ptr".to_string()),
            Type::Array(ty, n) => {
                let llvm_ty = self.llvm_type(&ty);
                Some(format!("[{} x {}]", n, llvm_ty.unwrap().as_str()))
            },
            Type::Void => Some("void".to_string()),
            Type::Ref(_) => Some("ptr".to_string()),
            Type::Generic(n, _) => Some(format!("%struct.{}", n)),
            Type::Named(name) => {
                let mut types = String::from(""); //String::from("{ ");
                let strct = self.aliases.get(&name).unwrap();
                for (_, ty) in &strct.fields {
                    types.push_str(format!("{}, ", self.llvm_type(ty).unwrap()).as_str())
                }
                types = types.strip_suffix(", ").unwrap_or(&types).to_string();
                // types.push_str(" }");
                Some(types)
            },
            Type::Pointer(_) => Some("ptr".to_string()),
        }
    }
}
