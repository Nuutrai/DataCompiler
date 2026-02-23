use crate::{
    ast::lexer::TextSpan,
    ast::parser::{AssignTarget, Expr, Statement, Type},
    codegen::{type_resolver::TypeResolver, value::Value},
    error::{ErrorKind, ErrorReporter},
};
use std::collections::{HashMap, HashSet};

pub struct AsmOutput {
    pub path: String,
    pub body: String,
    pub name: String,
    pub args: Vec<String>,
}

pub struct Codegen {
    output: String,
    globals: String,
    tmp: usize,
    types: TypeResolver,
    asm_outputs: Vec<AsmOutput>,
    libs: Vec<&'static str>,
    imported: HashSet<String>,
    pub errors: ErrorReporter,
    global_symbols: HashSet<String>,
    global_types: HashMap<String, Type>,
    scopes: Vec<HashMap<String, Type>>,
}

enum Storage {
    Local,
    Global,
}

impl Codegen {
    pub fn new(file: &str) -> Self {
        let libs = if cfg!(target_os = "windows") {
            vec!["-lkernel32", "-Wl,-subsystem,console"]
        } else if cfg!(target_os = "macos") {
            vec!["-lSystem"]
        } else {
            vec!["-lc"]
        };
        Self {
            output: String::new(),
            globals: String::new(),
            tmp: 0,
            types: TypeResolver::new(),
            asm_outputs: Vec::new(),
            libs,
            imported: HashSet::new(),
            errors: ErrorReporter::new(file),
            global_symbols: HashSet::new(),
            global_types: HashMap::new(),
            scopes: vec![HashMap::new()],
        }
    }

    fn fresh(&mut self) -> String {
        let name = format!("%t{}", self.tmp);
        self.tmp += 1;
        name
    }

    fn emit(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }

    fn platform() -> &'static str {
        if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "mac"
        } else {
            "linux"
        }
    }

    fn llvm_type(&mut self, ty: &Type) -> String {
        self.types.llvm_type(ty).unwrap_or_else(|| {
            self.errors
                .error_no_loc(ErrorKind::UnknownType(format!("{:?}", ty)));
            "i8".to_string()
        })
    }

    fn const_bytes(&mut self, expr: &Expr) -> Option<Vec<u8>> {
        match expr {
            Expr::String(s, _) => {
                let mut v = s.as_bytes().to_vec();
                v.push(0);
                Some(v)
            }

            Expr::Number(n, _) => Some(vec![*n as u8]),

            _ => None,
        }
    }

    fn error_not_defined(&mut self, name: &str, span: &TextSpan) {
        self.errors.error(
            ErrorKind::NotDefined(name.to_string()),
            span.line,
            span.start,
            name.len(),
        );
    }

    fn error_already_defined(&mut self, name: &str, span: &TextSpan) {
        self.errors.error(
            ErrorKind::AlreadyDefined(name.to_string()),
            span.line,
            span.start,
            name.len(),
        );
    }
}

impl Codegen {
    pub fn generate(&mut self, statements: &[Statement]) -> String {
        self.register_globals(statements);
        for stmt in statements {
            self.gen_statement(stmt);
        }
        let mut result = String::new();
        result.push_str("target triple = \"x86_64-w64-windows-gnu\"\n\n");
        result.push_str("declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n");
        result.push_str(&self.globals);
        result.push('\n');
        result.push_str(&self.output);
        result
    }

    fn register_globals(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            match stmt {
                Statement::Function { name, .. } => {
                    self.global_symbols.insert(name.clone());
                }
                Statement::AsmFunction { name, .. } => {
                    self.global_symbols.insert(name.clone());
                }
                Statement::Variable { name, .. } => {
                    self.global_symbols.insert(name.clone());
                }
                _ => {}
            }
        }
    }

    fn load_and_parse(&self, path: &str) -> Vec<Statement> {
        let input = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Cannot import '{}': file not found", path));
        crate::ast::parser::Parser::new(crate::ast::lexer::Lexer::new(input).tokenize(), path)
            .parse()
    }

    fn resolve_import(&self, path: &str) -> String {
        if !path.starts_with("std::") {
            return path.to_string();
        }
        let parts: Vec<&str> = path.split("::").collect();
        match parts.as_slice() {
            ["std", "common", rest @ ..] => format!("std/common/{}.dt", rest.join("/")),
            ["std", module] => format!("std/{}/{}.dt", Self::platform(), module),
            ["std", rest @ ..] => format!("std/{}/{}.dt", Self::platform(), rest.join("/")),
            _ => path.to_string(),
        }
    }

    fn lookup_local(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    fn lookup_variable(&self, name: &str) -> Option<(Type, Storage)> {
        if let Some(ty) = self.lookup_local(name) {
            return Some((ty, Storage::Local));
        }

        if let Some(ty) = self.global_types.get(name) {
            return Some((ty.clone(), Storage::Global));
        }

        None
    }

    fn emit_var_access(&mut self, name: &str, ty: Type, storage: Storage) -> Value {
        let prefix = match storage {
            Storage::Local => "%",
            Storage::Global => "@",
        };

        match ty {
            Type::DataArray(n) => {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {} = getelementptr [{} x i8], ptr {}{}, i32 0, i32 0",
                    tmp, n, prefix, name
                ));
                Value::new(&tmp, "ptr")
            }

            Type::Data => {
                let tmp = self.fresh();
                self.emit(&format!("  {} = load i8, ptr {}{}", tmp, prefix, name));
                Value::new(&tmp, "i8")
            }

            Type::Named(n) => {
                todo!("named type lookup for {}", n)
            }
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn current_scope(&mut self) -> &mut HashMap<String, Type> {
        self.scopes.last_mut().unwrap()
    }
}

impl Codegen {
    fn gen_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Function {
                name, args, body, ..
            } => self.gen_function(name, args, body),
            Statement::AsmFunction {
                name, args, body, ..
            } => self.gen_asm_function(name, args, body),
            Statement::Import(path, ..) => self.gen_import(path),
            Statement::Variable {
                name, ty, value, ..
            } => {
                self.global_symbols.insert(name.clone());

                if let Some(t) = ty {
                    self.global_types.insert(name.clone(), t.clone());
                }

                match (ty, value) {
                    (Some(Type::DataArray(n)), value) => {
                        if let Some(mut bytes) = self.const_bytes(value) {
                            if bytes.len() > *n {
                                bytes.truncate(*n);
                            } else if bytes.len() < *n {
                                bytes.resize(*n, 0);
                            }

                            let mut llvm_str = String::new();
                            for b in bytes {
                                if b >= 32 && b <= 126 && b != b'"' && b != b'\\' {
                                    llvm_str.push(b as char);
                                } else {
                                    llvm_str.push_str(&format!("\\{:02X}", b));
                                }
                            }

                            self.globals.push_str(&format!(
                                "@{} = global [{} x i8] c\"{}\"\n",
                                name, n, llvm_str
                            ));
                        } else {
                            self.errors.error_no_loc(ErrorKind::UnsupportedFeature(
                                "non-constant global initializer".to_string(),
                            ));
                        }
                    }

                    _ => {
                        let llvm_ty = ty
                            .as_ref()
                            .map(|t| self.llvm_type(t))
                            .unwrap_or("i8".to_string());

                        self.globals
                            .push_str(&format!("@{} = global {} zeroinitializer\n", name, llvm_ty));
                    }
                }
            }
            Statement::Expr(e) => {
                self.gen_expr(e);
            }
            Statement::Assign { .. } => {}
        }
    }

    fn gen_function(&mut self, name: &str, args: &[(String, Type)], body: &[Statement]) {
        self.global_symbols.insert(name.to_string());
        self.scopes.clear();
        self.enter_scope();
        let arg_str: Vec<String> = args
            .iter()
            .map(|(n, t)| {
                let ty = self.llvm_type(t);
                self.current_scope().insert(n.clone(), t.clone());
                format!("{} %{}_arg", ty, n)
            })
            .collect();
        self.emit(&format!("define i8 @{}({}) {{", name, arg_str.join(", ")));
        self.emit("entry:");
        for (n, t) in args {
            let ty = self.llvm_type(t);
            self.emit(&format!("  %{} = alloca {}", n, ty));
            self.emit(&format!("  store {} %{}_arg, ptr %{}", ty, n, n));
        }
        let mut last = Value::new("0", "i8");
        for s in body {
            last = self.gen_stmt_inner(s);
        }
        self.emit(&format!("  ret {} {}", last.ty, last.name));
        self.emit("}");
        self.emit("");
    }

    fn gen_asm_function(&mut self, name: &str, args: &[(String, Type)], body: &str) {
        self.global_symbols.insert(name.to_string());
        let arg_types: Vec<String> = args.iter().map(|(_, t)| self.llvm_type(t)).collect();
        self.emit(&format!("declare i8 @{}({})", name, arg_types.join(", ")));
        self.emit("");
        self.asm_outputs.push(AsmOutput {
            path: format!("{}.s", name),
            body: format!(
                "    .intel_syntax noprefix\n    .globl {}\n{}:\n{}\n    .att_syntax prefix",
                name,
                name,
                body.lines()
                    .map(|l| format!("    {}", l))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            name: name.to_string(),
            args: arg_types,
        });
    }

    fn gen_import(&mut self, path: &str) {
        let resolved = self.resolve_import(path);
        if self.imported.contains(&resolved) {
            return;
        }
        self.imported.insert(resolved.clone());
        let ast = self.load_and_parse(&resolved);
        self.register_globals(&ast);
        for stmt in ast {
            self.gen_statement(&stmt);
        }
    }
}

impl Codegen {
    fn gen_stmt_inner(&mut self, stmt: &Statement) -> Value {
        match stmt {
            Statement::Variable {
                name,
                ty,
                value,
                span,
            } => self.gen_variable(name, ty, value, span),
            Statement::Assign {
                target,
                value,
                span,
            } => self.gen_assign(target, value, span),
            Statement::Expr(e) => self.gen_expr(e),
            _ => Value::new("0", "i8"),
        }
    }

    fn gen_variable(
        &mut self,
        name: &str,
        ty: &Option<Type>,
        value: &Expr,
        span: &TextSpan,
    ) -> Value {
        if self.current_scope().contains_key(name) {
            self.error_already_defined(name, span);
            return Value::new("0", "i8");
        }
        let val = self.gen_expr(value);
        let resolved_ty = ty.clone().unwrap_or_else(|| match val.ty.as_str() {
            "ptr" => Type::DataArray(0),
            _ => Type::Data,
        });
        let llvm_ty = self.llvm_type(&resolved_ty);
        self.current_scope()
            .insert(name.to_string(), resolved_ty.clone());
        self.emit(&format!("  %{} = alloca {}", name, llvm_ty));
        self.alloc_store(name, &llvm_ty, &val, ty);
        Value::new(&format!("%{}", name), &llvm_ty)
    }

    fn alloc_store(&mut self, name: &str, llvm_ty: &str, val: &Value, src_ty: &Option<Type>) {
        match src_ty {
            Some(Type::DataArray(n)) if *n > 0 && val.ty == "ptr" => {
                self.emit(&format!(
                    "  call void @llvm.memcpy.p0.p0.i64(ptr %{}, ptr {}, i64 {}, i1 false)",
                    name, val.name, n
                ));
            }
            _ => self.emit(&format!("  store {} {}, ptr %{}", llvm_ty, val.name, name)),
        }
    }

    fn gen_assign(&mut self, target: &AssignTarget, value: &Expr, span: &TextSpan) -> Value {
        let val = self.gen_expr(value);
        match target {
            AssignTarget::Variable(name) => self.gen_assign_var(name, val, span),
            AssignTarget::Index(name, index) => self.gen_assign_index(name, index, val),
        }
    }

    fn gen_assign_var(&mut self, name: &str, val: Value, span: &TextSpan) -> Value {
        let ty = match self.lookup_local(name) {
            Some(t) => t,
            None => {
                self.error_not_defined(name, span);
                return Value::new("0", "i8");
            }
        };
        let llvm_ty = self.llvm_type(&ty);
        self.emit(&format!("  store {} {}, ptr %{}", llvm_ty, val.name, name));
        Value::new(&format!("%{}", name), &llvm_ty)
    }

    fn gen_assign_index(&mut self, name: &str, index: &Expr, val: Value) -> Value {
        let idx = self.gen_expr(index);
        let gep = self.fresh();
        self.emit(&format!(
            "  {} = getelementptr i8, ptr %{}, i64 {}",
            gep, name, idx.name
        ));
        self.emit(&format!("  store i8 {}, ptr {}", val.name, gep));
        Value::new(&val.name, "i8")
    }
}

impl Codegen {
    fn gen_expr(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Number(n, _) => Value::new(&n.to_string(), "i8"),
            Expr::String(s, _) => self.gen_string(s),
            Expr::Identifier(name, span) => self.gen_identifier(name, span),
            Expr::Group(inner, _) => self.gen_expr(inner),
            Expr::Call { callee, args, .. } => self.gen_call(callee, args),
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
                ..
            } => self.gen_ternary(cond, then_branch, else_branch),
            Expr::Index { target, index, .. } => self.gen_index(target, index),
        }
    }

    fn gen_string(&mut self, s: &str) -> Value {
        let global_name = format!("@str{}", self.tmp);
        self.tmp += 1;
        let len = s.len() + 1;
        self.globals.push_str(&format!(
            "{} = private constant [{} x i8] c\"{}\\00\"\n",
            global_name, len, s
        ));
        let tmp = self.fresh();
        self.emit(&format!(
            "  {} = getelementptr [{} x i8], ptr {}, i32 0, i32 0",
            tmp, len, global_name
        ));
        Value::new(&tmp, "ptr")
    }

    fn gen_identifier(&mut self, name: &str, span: &TextSpan) -> Value {
        if let Some((ty, storage)) = self.lookup_variable(name) {
            return self.emit_var_access(name, ty, storage);
        }

        self.error_not_defined(name, span);
        Value::new("0", "i8")
    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr]) -> Value {
        let arg_vals: Vec<String> = args.iter().map(|a| self.gen_expr(a).as_arg()).collect();
        let callee_name = match callee {
            Expr::Identifier(n, span) => {
                if !self.global_symbols.contains(n) {
                    self.error_not_defined(n, span);
                }
                n.clone()
            }
            _ => {
                self.errors.error_no_loc(ErrorKind::UnsupportedFeature(
                    "complex callees not yet supported".to_string(),
                ));
                return Value::new("0", "i8");
            }
        };
        let tmp = self.fresh();
        self.emit(&format!(
            "  {} = call i8 @{}({})",
            tmp,
            callee_name,
            arg_vals.join(", ")
        ));
        Value::new(&tmp, "i8")
    }

    fn gen_ternary(&mut self, cond: &Expr, then_branch: &Expr, else_branch: &Expr) -> Value {
        let cond_val = self.gen_expr(cond);
        let then_label = format!("then{}", self.tmp);
        let else_label = format!("else{}", self.tmp);
        let merge_label = format!("merge{}", self.tmp);
        self.tmp += 1;
        let cond_bit = self.fresh();
        self.emit(&format!("  {} = icmp ne i8 {}, 0", cond_bit, cond_val.name));
        self.emit(&format!(
            "  br i1 {}, label %{}, label %{}",
            cond_bit, then_label, else_label
        ));
        self.emit(&format!("{}:", then_label));
        let then_val = self.gen_expr(then_branch);
        self.emit(&format!("  br label %{}", merge_label));
        self.emit(&format!("{}:", else_label));
        let else_val = self.gen_expr(else_branch);
        self.emit(&format!("  br label %{}", merge_label));
        self.emit(&format!("{}:", merge_label));
        let result = self.fresh();
        self.emit(&format!(
            "  {} = phi {} [ {}, %{} ], [ {}, %{} ]",
            result, then_val.ty, then_val.name, then_label, else_val.name, else_label
        ));
        Value::new(&result, &then_val.ty)
    }

    fn gen_index(&mut self, target: &Expr, index: &Expr) -> Value {
        let ptr = self.gen_expr(target);
        let idx = self.gen_expr(index);
        let gep = self.fresh();
        let tmp = self.fresh();
        self.emit(&format!(
            "  {} = getelementptr i8, ptr {}, i64 {}",
            gep, ptr.name, idx.name
        ));
        self.emit(&format!("  {} = load i8, ptr {}", tmp, gep));
        Value::new(&tmp, "i8")
    }
}

impl Codegen {
    pub fn compile_to_binary(&self, ir: &str, out: &str) {
        std::fs::write("out.ll", ir).unwrap();
        std::process::Command::new("clang")
            .args(["-c", "out.ll", "-o", "out.o"])
            .status()
            .expect("IR compile failed");
        let mut obj_files = vec!["out.o".to_string()];
        for asm in &self.asm_outputs {
            std::fs::write(&asm.path, &asm.body).unwrap();
            let obj = asm.path.replace(".s", ".o");
            std::process::Command::new("clang")
                .args(["-c", &asm.path, "-o", &obj])
                .status()
                .expect("ASM compile failed");
            obj_files.push(obj);
        }
        let mut cmd = std::process::Command::new("clang");
        for obj in &obj_files {
            cmd.arg(obj);
        }
        for lib in &self.libs {
            cmd.arg(lib);
        }
        cmd.arg("-o").arg(out);
        cmd.status().expect("Link failed");
    }
}
