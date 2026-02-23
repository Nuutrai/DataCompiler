use crate::ast::parser::Type;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum Storage {
    Local,
    Global,
}

pub struct ScopeStack {
    scopes: Vec<HashMap<String, Type>>,
    global_types: HashMap<String, Type>,
    global_symbols: HashSet<String>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            global_types: HashMap::new(),
            global_symbols: HashSet::new(),
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
        self.global_symbols.insert(name.to_string());
    }

    pub fn register_symbol(&mut self, name: &str) {
        self.global_symbols.insert(name.to_string());
    }

    pub fn is_known_symbol(&self, name: &str) -> bool {
        self.global_symbols.contains(name)
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
}
