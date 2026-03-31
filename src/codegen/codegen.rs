use crate::{
    ast::{
        lexer::TextSpan,
        parser::{AssignTarget, Expr, Statement, Type},
    },
    codegen::{
        compiler::Compiler,
        emitter::Emitter,
        scope::{ScopeStack, Storage, field_size},
        type_resolver::TypeResolver,
        value::Value,
    },
    error::{ErrorKind, ErrorReporter},
};
use std::collections::HashSet;
use std::fmt::format;
use std::ops::Deref;
use std::thread::scope;
use std::vec;
use crate::codegen::scope::SymbolType;

pub struct Codegen {
    emitter: Emitter,
    scope: ScopeStack,
    compiler: Compiler,
    types: TypeResolver,
    imported: HashSet<String>,
    target: String,
    pub errors: ErrorReporter,
}

impl Codegen {
    pub fn new(file: &str, target: &str) -> Self {
        Self {
            emitter: Emitter::new(),
            scope: ScopeStack::new(),
            compiler: Compiler::new(target),
            types: TypeResolver::new(),
            imported: HashSet::new(),
            target: target.to_string(),
            errors: ErrorReporter::new(file),
        }
    }

    fn llvm_type(&mut self, ty: &Type) -> String {
        self.types.llvm_type(ty).unwrap_or_else(|| {
            self.errors
                .error_no_loc(ErrorKind::UnknownType(format!("{:?}", ty)));
            "i8".to_string() // TODO i8
        })
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

    fn err_not_defined(&mut self, name: &str, span: &TextSpan) {
        self.errors.error(
            ErrorKind::NotDefined(name.to_string()),
            span.line,
            span.start,
            name.len(),
        );
    }

    fn err_already_defined(&mut self, name: &str, span: &TextSpan) {
        self.errors.error(
            ErrorKind::AlreadyDefined(name.to_string()),
            span.line,
            span.start,
            name.len(),
        );
    }
}

impl Codegen {
    pub fn generate(&mut self, stmts: &[Statement]) -> String {
        self.register_globals(stmts);
        for stmt in stmts {
            self.gen_statement(stmt);
        }
        let (globals, output) = self.emitter.finish_ref();
        let mut result = String::new();
        result.push_str(format!("target triple = \"{}\"\n\n", self.target).as_str());
        result.push_str("declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n"); // TODO i64
        result.push_str(globals);
        result.push('\n');
        result.push_str(output);
        result
    }

    pub fn compile_to_binary(&self, ir: &str, out: &str) {
        self.compiler.compile(ir, out, self.target.as_str());
    }

    fn register_globals(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            match stmt {
                Statement::Function { name, returns, .. } => self.scope.register_symbol(name, returns),
                Statement::AsmFunction { arch, name, returns, .. } if arch.eq(&self.target) => {
                    self.scope.register_symbol(
                        name,
                        match returns {
                            Some(_) => returns,
                            None => &Some(Type::Void)
                        }
                    )
                },
                Statement::Struct { name, .. } => {
                    self.scope.register_symbol(name, &None);
                },
                Statement::Variable { name, ty, .. } => {
                    if let Some(t) = ty {
                        self.scope.insert_global(name, t.clone());
                    } else {
                        self.scope.register_symbol(name, &None);
                    }
                }
                _ => {}
            }
        }
    }

    fn resolve_import(&self, path: &str) -> String {
        if !path.starts_with("std::") {
            return path.to_string();
        }
        let parts: Vec<&str> = path.split("::").collect();
        match parts.as_slice() {
            // ["std", "common", rest @ ..] => format!("std/common/{}.dt", rest.join("/")),
            ["std", module] => format!("std/{}.dt", module),
            ["std", rest @ ..] => format!("std/{}.dt", rest.join("/")),
            _ => path.to_string(),
        }
    }

    fn load_and_parse(&self, path: &str) -> Vec<Statement> {
        let input = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Cannot import '{}': file not found", path));
        crate::ast::parser::Parser::new(crate::ast::lexer::Lexer::new(input).tokenize(), path)
            .parse()
    }
}

impl Codegen {
    fn gen_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Function {
                name, args, returns, body, ..
            } => self.gen_function(name, args, returns, body),
            Statement::AsmFunction {
                arch, name, args, returns, body, ..
            } if (arch.eq(&self.target)) => self.gen_asm_function(name, args, returns, body),
            Statement::Import(path, ..) => self.gen_import(path),
            Statement::Variable {
                name, ty, value, span
            } => {
                match value {
                    Some(value) => self.gen_global_var(name, ty, &value),
                    None => self.errors.error(ErrorKind::NotDefined(name.clone()), span.line, span.start, span.end-span.start+1)
                }
            },
            Statement::Expr(e) => {
                self.gen_expr(e);
            }
            Statement::Struct {
                name,
                generics,
                fields,
                ..
            } => self.gen_struct(name, generics, fields),
            Statement::Assign { .. } => {}
            _ => {}
        }
    }

    fn gen_function(&mut self, name: &str, args: &[(String, Type)], returns: &Option<Type>, body: &[Statement]) {
        self.scope.register_symbol(
            name,
            match returns {
                Some(_) => returns,
                None => &Some(Type::Void)
            }
        );
        self.scope.clear_locals();
        let arg_str: Vec<String> = args
            .iter()
            .map(|(n, t)| {
                let ty = self.llvm_type(t);
                self.scope.insert_local(n, t.clone());
                format!("{} %{}_arg", ty, n)
            })
            .collect();
        let returns = self.llvm_type(
            match returns {
                Some(ty) => ty,
                None => &Type::Void
            }
        );
        self.emitter
            .emit(&format!("define {} @{}({}) {{", returns, name, arg_str.join(", ")));
        self.emitter.emit("entry:");
        for (n, t) in args {
            let ty = self.llvm_type(t);
            self.emitter.emit(&format!("  %{} = alloca {}", n, ty));
            self.emitter
                .emit(&format!("  store {} %{}_arg, ptr %{}", ty, n, n));
        }
        let mut last = Value::new("0", &returns);
        for s in body {
            last = self.gen_stmt_inner(s);
        }
        if returns == "void" {
            self.emitter
                .emit(&format!("  ret {}", returns));

        } else {
        self.emitter
            .emit(&format!("  ret {} {}", returns, last.name));
        }
        self.emitter.emit("}");
        self.emitter.emit("");
    }

    fn gen_asm_function(&mut self, name: &str, args: &[Type], returns: &Option<Type>, body: &str) {
        self.scope.register_symbol(
            name,
            match returns {
                Some(_) => returns,
                None => &Some(Type::Void)
            }
        );
        let arg_types: Vec<String> = args.iter().map(|(t)| self.llvm_type(t)).collect();
        let returns = self.llvm_type(
            match returns {
                Some(ty) => ty,
                None => &Type::Void
            }
        );
        self.emitter
            .emit(&format!("declare {} @{}({})", returns, name, arg_types.join(", ")));
        self.emitter.emit("");
        let asm_body = format!(
            "    .intel_syntax noprefix\n    .text\n.globl _{}\n_{}:\n{}\n    .att_syntax prefix",
            name,
            name,
            body.lines()
                .map(|l| format!("    {}", l))
                .collect::<Vec<_>>()
                .join("\n")
        );
        self.compiler.add_asm(format!("{}.s", name), asm_body);
    }

    fn gen_field(&mut self, target: &Expr, field: &str, span: &TextSpan) -> Value {
        let ptr = self.gen_expr(target).into_iter().nth(0).unwrap();

        let struct_name = match self.expr_struct_name(target) {
            Some(n) => n,
            None => {
                self.errors.error_no_loc(ErrorKind::UnsupportedFeature(
                    "field access on non-struct type".to_string(),
                ));
                return Value::new("0", "i8"); // TODO i8
            }
        };

        let (offset, field_ty) = match self.scope.field_offset(&struct_name, field) {
            Some(r) => r,
            None => {
                self.errors.error(
                    ErrorKind::NotDefined(format!("{}.{}", struct_name, field)),
                    span.line,
                    span.start,
                    field.len(),
                );
                return Value::new("0", "i8"); // TODO i8
            }
        };

        let llvm_ty = self.llvm_type(&field_ty);
        let gep = self.emitter.fresh();
        let tmp = self.emitter.fresh();
        self.emitter.emit(&format!(
            "  {} = getelementptr i8, ptr {}, i64 {}", // TODO i64
            gep, ptr.name, offset
        ));
        self.emitter
            .emit(&format!("  {} = load {}, ptr {}", tmp, llvm_ty, gep));
        Value::new(&tmp, &llvm_ty)
    }

    fn gen_struct(&mut self, name: &str, generics: &[String], fields: &[(String, Type)]) {
        self.scope
            .register_struct(name, generics.to_vec(), fields.to_vec());

        self.types.register(name.to_string(), self.scope.get_struct(name).unwrap().clone());

        let types: Vec<String> = fields.iter().map(|(_, t)| self.llvm_type(t)).collect();
        self.emitter.emit_global(&format!(
            "%struct.{} = type {{ {} }}",
            name, types.join(", ")
        ));
    }

    fn expr_struct_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name, _) => match self.scope.lookup(name) {
                Some((Type::Named(n), _)) => Some(n),
                Some((Type::Generic(n, _), _)) => Some(n),
                _ => None,
            },
            _ => None,
        }
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

    fn gen_global_var(&mut self, name: &str, ty: &Option<Type>, value: &Expr) {
        if let Some(t) = ty {
            self.scope.insert_global(name, t.clone());
        } else {
            self.scope.register_symbol(name, &None);
        }
        match ty {
            Some(Type::DataArray(bits, n)) => {
                let bytes = self.const_bytes_padded(value, *n);
                self.emitter
                    .emit_global(&format!("@{} = global [{} x i8] c\"{}\"", name, n, bytes));  // TODO i8
            }
            _ => {
                let llvm_ty = ty
                    .as_ref()
                    .map(|t| self.llvm_type(t))
                    .unwrap_or("i8".to_string()); // TODO i8
                self.emitter
                    .emit_global(&format!("@{} = global {} zeroinitializer", name, llvm_ty));
            }
        }
    }

    fn const_bytes_padded(&mut self, expr: &Expr, size: usize) -> String {
        let mut bytes = self.const_bytes(expr).unwrap_or_else(|| {
            self.errors.error_no_loc(ErrorKind::UnsupportedFeature(
                "non-constant global initializer".to_string(),
            ));
            vec![]
        });
        bytes.resize(size, 0);
        bytes.truncate(size);
        bytes
            .iter()
            .map(|&b| {
                if (32..=126).contains(&b) && b != b'"' && b != b'\\' {  // TODO Maybe add an explanation for what's going on here lol
                    format!("{}", b as char)
                } else {
                    format!("\\{:02X}", b)
                }
            })
            .collect()
    }

    fn const_bytes(&self, expr: &Expr) -> Option<Vec<u8>> {
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
}

impl Codegen {
    fn gen_stmt_inner(&mut self, stmt: &Statement) -> Value {
        match stmt {
            Statement::Variable {
                name,
                ty,
                value,
                span,
            } => self.gen_local_var(name, ty, value, span),
            Statement::Assign {
                target,
                value,
                span,
            } => self.gen_assign(target, value, span),
            Statement::Struct { .. } => Value::new("0", "i8"),  // TODO i8
            Statement::Expr(e) => self.gen_expr(e).into_iter().next().unwrap(),
            _ => Value::new("0", "i8"),  // TODO i8
        }
    }

    fn gen_local_var(
        &mut self,
        name: &str,
        ty: &Option<Type>,
        value: &Option<Expr>,
        span: &TextSpan,
    ) -> Value {
        if self.scope.contains_local(name) {
            self.err_already_defined(name, span);
            return Value::new("0", "i8");  // TODO i8
        }

        let val = if let Some(value) = value {
            self.gen_expr(value)
        } else { Vec::new() };

        let resolved = ty.clone().unwrap_or_else(|| match val.iter().next() {
            Some(v) => {
                match value {
                    Some(Expr::String(s, _)) => {
                        Type::DataArray(8, val.len())
                    }
                    _ => {
                        match v.ty.as_str() {
                            "ptr" => Type::DataArray(8, 0),
                            _ => Type::Data(8),
                        }
                    }
                }

            }
            None => Type::Data(8)
        });
        let llvm_ty = self.llvm_type(&resolved);
        self.scope.insert_local(name, resolved.clone());
        self.emitter
            .emit(&format!("  %{} = alloca {}", name, llvm_ty));

        match resolved {
            Type::DataArray(bits, size) => {
                let mut llvm_val = String::from("[ ");
                for i in 0..size {
                    llvm_val.push_str(&format!("i{} {}, " , bits, val.get(i).unwrap_or(&Value::new("0", "")).name));
                }
                llvm_val = llvm_val.strip_suffix(", ").unwrap_or(&llvm_val).to_string();
                llvm_val.push_str(" ]");
                let mut value = Value::new(&llvm_val, &llvm_ty);
                self.alloc_store(name, &llvm_ty, &value, &None);
            }
            _ => {
                match val.into_iter().next() {
                    Some(v) => {
                        self.alloc_store(name, &llvm_ty, &v, &None);
                    }
                    None => {}
                }
            }
        }

        Value::new(&format!("%{}", name), &llvm_ty)
    }

    fn alloc_store(&mut self, name: &str, llvm_ty: &str, val: &Value, src_ty: &Option<Type>) {
        match src_ty {
            Some(Type::DataArray(bits, n)) if *n > 0 && val.ty == "ptr" => {
                self.emitter.emit(&format!(
                    "  call void @llvm.memcpy.p0.p0.i64(ptr %{}, ptr {}, i64 {}, i1 false)",  // TODO i64
                    name, val.name, n
                ));
            }
            _ => self
                .emitter
                .emit(&format!("  store {} {}, ptr %{}", llvm_ty, val.name, name)),
        }
    }

    fn gen_assign(&mut self, target: &AssignTarget, value: &Expr, span: &TextSpan) -> Value {
        let val = self.gen_expr(value).into_iter().nth(0).unwrap();
        if val.ty == "void" {
            return Value::new("0", "void");
        }
        match target {
            AssignTarget::Variable(name) => self.gen_assign_var(name, val, span),
            AssignTarget::Index(name, index) => self.gen_assign_index(name, index, val),
        }
    }

    // TODO When we run into issues with Void, we'll edit this
    fn gen_assign_var(&mut self, name: &str, val: Value, span: &TextSpan) -> Value {
        let ty = match self.scope.lookup(name) {
            Some((t, _)) => t,
            None => {
                self.err_not_defined(name, span);
                return Value::new("0", "i8");  // TODO i8
            }
        };
        let llvm_ty = self.llvm_type(&ty);
        self.emitter
            .emit(&format!("  store {} {}, ptr %{}", llvm_ty, val.name, name));
        Value::new(&format!("%{}", name), &llvm_ty)
    }

    fn gen_assign_index(&mut self, name: &str, index: &Expr, val: Value) -> Value {
        let idx = self.gen_expr(index).into_iter().nth(0).unwrap();
        let prefix = match self.scope.lookup(name) {
            Some((_, Storage::Global)) => "@",
            _ => "%",
        };
        let gep = self.emitter.fresh();
        self.emitter.emit(&format!(
            "  {} = getelementptr i8, ptr {}{}, i64 {}",  // TODO i64
            gep, prefix, name, idx.name
        ));
        self.emitter
            .emit(&format!("  store i8 {}, ptr {}", val.name, gep)); // TODO i8
        Value::new(&val.name, "i8") // TODO i8
    }
}

impl Codegen {
    fn gen_expr(&mut self, expr: &Expr) -> Vec<Value> {
        match expr {
            Expr::Number(n, _) =>  vec!(Value::new(&n.to_string(), "i8")), // TODO i8
            Expr::String(s, _) => self.gen_str(s),
            Expr::Identifier(name, span) => vec!(self.gen_identifier(name, span)),
            Expr::Group(inner, _) => self.gen_expr(inner),
            Expr::Field {
                target,
                field,
                span,
            } => vec!(self.gen_field(target, field, span)),
            Expr::Call { callee, args, .. } => vec!(self.gen_call(callee, args)),
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
                ..
            } => vec!(self.gen_ternary(cond, then_branch, else_branch)),
            Expr::Index { target, index, .. } => vec!(self.gen_index(target, index)),
            Expr::Pointer(e, _) => {
                vec!(self.gen_ptr(e))
            },
            Expr::List(exprs, _) => {
                self.gen_list(exprs)
            },
        }
    }

    fn gen_string(&mut self, s: &str) -> Value {
        let n = self.emitter.tmp_counter();
        let global_name = format!("@str{}", n);
        let len = s.len() + 1;
        self.emitter.emit_global(&format!(
            "{} = private constant [{} x i8] c\"{}\\00\"", // TODO i8
            global_name, len, s
        ));
        let tmp = self.emitter.fresh();
        self.emitter.emit(&format!(
            "  {} = getelementptr [{} x i8], ptr {}, i32 0, i32 0", // TODO i32
            tmp, len, global_name
        ));
        Value::new(&tmp, "ptr")
    }

    fn gen_str(&mut self, s: &str) -> Vec<Value> {
        let mut chars = Vec::new();
        for ch in s.as_bytes() {
            chars.push(Value::new(&format!("{}", ch), "i8"));
        }
        chars
    }

    fn gen_identifier(&mut self, name: &str, span: &TextSpan) -> Value {
        match self.scope.lookup(name) {
            Some((ty, storage)) => self.emit_var_access(name, ty, storage),
            None => {
                self.err_not_defined(name, span);
                Value::new("0", "i8") // TODO i8
            }
        }
    }

    fn emit_var_access(&mut self, name: &str, ty: Type, storage: Storage) -> Value {
        let prefix = match storage {
            Storage::Local => "%",
            Storage::Global => "@",
        };
        match &ty {
            Type::DataArray(bits, n) => {
                let tmp = self.emitter.fresh();
                self.emitter.emit(&format!(
                    "  {} = getelementptr [{} x i{}], ptr {}{}, i32 0, i32 0", // TODO i32
                    tmp, n, bits, prefix, name
                ));
                Value::new(&tmp, "ptr")
            }
            Type::Data(bits) => {
                let tmp = self.emitter.fresh();
                self.emitter
                    .emit(&format!("  {} = load i{}, ptr {}{}", tmp, bits, prefix, name));
                Value::new(&tmp, format!("i{}", bits).as_str())
            }
            Type::Pointer(inner) => {
                let tmp = self.emitter.fresh();
                self.emitter
                    .emit(&format!("  {} = load ptr, ptr {}{}", tmp, prefix, name));
                Value::new(&tmp, "ptr")
            },
            Type::Ref(_) => {
                let tmp = self.emitter.fresh();
                self.emitter
                    .emit(&format!("  {} = load ptr, ptr {}{}", tmp, prefix, name));
                Value::new(&tmp, "ptr")
            }
            Type::Generic(n, _) => {
                let tmp = self.emitter.fresh();
                self.emitter.emit(&format!(
                    "  {} = getelementptr %struct.{}, ptr {}{}, i32 0, i32 0", // TODO i32
                    tmp, n, prefix, name
                ));
                Value::new(&tmp, "ptr")
            }
            Type::Void => {
                Value::new("0", "void")
            }
            Type::Named(_) => {
                let tmp = self.emitter.fresh();
                let llvm_ty = self.llvm_type(&ty);
                self.emitter.emit(
                    &format!("  {} = load {}, ptr {}{}", tmp, llvm_ty.as_str(), prefix, name)
                );
                Value::new(&tmp, llvm_ty.as_str())
            },
        }
    }

    fn gen_ptr(&mut self, inner: &Expr) -> Value {
        match inner {
            Expr::Identifier(name, span) => {
                match self.scope.lookup(name) {
                    Some((ty, storage)) => {
                        // let tmp = self.emitter.fresh();
                        // self.emitter.emit(&format!(
                        //     "  {} = alloca ptr \n  store ptr %{}, ptr {}", tmp, name, tmp
                        // ));
                        Value::new(&format!("{}{name}", match storage { Storage::Global => "@", _ => "%" }), "ptr")
                    }
                    None => {
                        self.err_not_defined(name, span);
                        Value::new("0", "i8") // TODO i8
                    }
                }
            }
            _ => {

                let expr = self.gen_expr(inner).into_iter().nth(0).unwrap();

                let tmp = self.emitter.fresh();
                self.emitter.emit(&format!(
                    "  {} = alloca {} \n  store {} {}, ptr {}", tmp, expr.ty, expr.ty, expr.name, tmp
                ));

                Value::new(&tmp, "ptr")
            }
        }

    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr]) -> Value {
        let arg_vals: Vec<String> = args.iter().map(|a| self.gen_expr(a).first().unwrap().as_arg()).collect();
        let name = match callee {
            Expr::Identifier(n, span) => {
                if !self.scope.is_known_symbol(n) {
                    self.err_not_defined(n, span);
                }
                n.clone()
            }
            _ => {
                self.errors.error_no_loc(ErrorKind::UnsupportedFeature(
                    "complex callees not yet supported".to_string(),
                ));
                return Value::new("0", "i8"); // TODO i8
            }
        };
        let returns = match self.scope.get_symbol_type(&name) {
            Some(symty) => {
                match symty {
                    SymbolType::Typed(ty) => ty,
                    SymbolType::Untyped => &Type::Data(8),
                }
            },
            None => &Type::Data(8),
        };
        let ty = self.types.llvm_type(returns).unwrap_or("i8".to_string());
        if ty == "void" {
            self.emitter.emit(&format!(
                "  call {} @{}({})",
                ty,
                name,
                arg_vals.join(", ")
            ));
            Value::new("0", &ty)

        } else {
            let tmp = self.emitter.fresh();
            self.emitter.emit(&format!(
                "  {} = call {} @{}({})",
                tmp,
                ty,
                name,
                arg_vals.join(", ")
            ));
            Value::new(&tmp, &ty)
        }
    }

    fn gen_ternary(&mut self, cond: &Expr, then_b: &Expr, else_b: &Expr) -> Value {
        let cond_val = self.gen_expr(cond).into_iter().nth(0).unwrap();
        let n = self.emitter.tmp_counter();
        let (then_l, else_l, merge_l) = (
            format!("then{}", n),
            format!("else{}", n),
            format!("merge{}", n),
        );
        let cond_bit = self.emitter.fresh();
        self.emitter
            .emit(&format!("  {} = icmp ne i8 {}, 0", cond_bit, cond_val.name)); // TODO i8
        self.emitter.emit(&format!(
            "  br i1 {}, label %{}, label %{}",
            cond_bit, then_l, else_l
        ));
        self.emitter.emit(&format!("{}:", then_l));
        let then_val = self.gen_expr(then_b).into_iter().nth(0).unwrap();
        self.emitter.emit(&format!("  br label %{}", merge_l));
        self.emitter.emit(&format!("{}:", else_l));
        let else_val = self.gen_expr(else_b).into_iter().nth(0).unwrap();
        self.emitter.emit(&format!("  br label %{}", merge_l));
        self.emitter.emit(&format!("{}:", merge_l));
        let result = self.emitter.fresh();
        self.emitter.emit(&format!(
            "  {} = phi {} [ {}, %{} ], [ {}, %{} ]",
            result, then_val.ty, then_val.name, then_l, else_val.name, else_l
        ));
        Value::new(&result, &then_val.ty)
    }

    fn gen_index(&mut self, target: &Expr, index: &Expr) -> Value {

        let ptr = self.gen_expr(target).into_iter().nth(0).unwrap();
        let idx = self.gen_expr(index).into_iter().nth(0).unwrap();

        let gep = self.emitter.fresh();
        let tmp = self.emitter.fresh();
        self.emitter.emit(&format!(
            "  {} = getelementptr i8, ptr {}, i64 {}", // TODO i64
            gep, ptr.name, idx.name
        ));
        self.emitter
            .emit(&format!("  {} = load i8, ptr {}", tmp, gep)); // TODO i8
        Value::new(&tmp, "i8") // TODO i8
    }

    fn gen_list(&mut self, exprs: &Vec<Expr>) -> Vec<Value> {
        let mut vals = Vec::new();
        for expr in exprs {
            for expr in self.gen_expr(expr) {
                vals.push(expr);
            }
        }
        vals
    }
}
