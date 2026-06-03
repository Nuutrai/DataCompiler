use crate::ast::parser::Type;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum Storage {
    Local,
    Global,
}

pub enum SymbolType {
    Typed(Type),
    Untyped,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub fields: Vec<(String, Type)>,
    pub generics: Vec<String>,
}

pub struct ScopeStack {
    scopes: Vec<HashMap<String, Type>>,
    global_types: HashMap<String, Type>,
    global_symbols: HashMap<String, SymbolType>,
    pub structs: HashMap<String, StructDef>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            global_types: HashMap::new(),
            global_symbols: HashMap::new(),
            structs: HashMap::new(),
        }
    }

    pub fn enter(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit(&mut self) {
        self.scopes.pop();
    }

    pub fn clear_locals(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
    }

    pub fn current(&mut self) -> &mut HashMap<String, Type> {
        self.scopes.last_mut().unwrap()
    }

    pub fn insert_local(&mut self, name: &str, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), ty);
    }

    pub fn insert_global(&mut self, name: &str, ty: Type) {
        self.global_types.insert(name.to_string(), ty);
        self.global_symbols.insert(name.to_string(), SymbolType::Untyped);
    }

    pub fn register_symbol(&mut self, name: &str, ty: &Option<Type>) {
        self.global_symbols.insert(
            name.to_string(),
            match ty {
                None => SymbolType::Untyped,
                Some(ty) => {
                    SymbolType::Typed(ty.clone())
                }
            }
        );
    }

    pub fn is_known_symbol(&self, name: &str) -> bool {
        self.global_symbols.contains_key(name)

    }

    pub fn contains_local(&self, name: &str) -> bool {
        self.scopes
            .last()
            .map(|s| s.contains_key(name))
            .unwrap_or(false)
    }

    pub fn lookup(&self, name: &str) -> Option<(Type, Storage)> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some((ty.clone(), Storage::Local));
            }
        }
        if let Some(ty) = self.global_types.get(name) {
            return Some((ty.clone(), Storage::Global));
        }
        None
    }

    pub fn get_symbol_type(&self, name: &str) -> Option<&SymbolType> {
        self.global_symbols.get(name)
    }

    pub fn register_struct(
        &mut self,
        name: &str,
        generics: Vec<String>,
        fields: Vec<(String, Type)>,
    ) {
        self.structs
            .insert(name.to_string(), StructDef { fields, generics });
    }

    pub fn get_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.get(name)
    }

    pub fn field_offset(&self, struct_name: &str, field: &str) -> Option<(usize, Type)> {
        let def = self.structs.get(struct_name)?;
        let mut offset = 0;
        for (fname, ftype) in &def.fields {
            if fname == field {
                return Some((offset, ftype.clone()));
            }
            offset += self.field_size(ftype);
        }
        None
    }

    pub fn field_size(&self, ty: &Type) -> usize {
        match ty {
            Type::Data(bits) => (bits / 8) as usize,
            Type::Array(ty, n) => n * (self.field_size(ty) / 8),
            Type::Pointer(_) => 8,
            Type::Named(name) => {
                let def = self.structs.get(name).unwrap(); // TODO Add proper error handling
                let mut offset = 0;
                for (_, ty) in &def.fields {
                    offset += self.field_size(ty);
                }
                offset
            }
            Type::Generic(name, types) => {
                let mut offset = 0;
                let def = self.structs.get(name).unwrap();
                for (_, ty) in &def.fields {

                    offset += self.field_size(ty);
                }
                offset
            }
            _ => {
                8
            },
        }
    }
}


