use std::process::Command;

pub struct AsmOutput {
    pub path: String,
    pub body: String,
}

pub struct Compiler {
    libs: Vec<&'static str>,
    asm_outputs: Vec<AsmOutput>,
}

impl Compiler {
    pub fn new() -> Self {
        let libs = if cfg!(target_os = "windows") {
            vec!["-lkernel32", "-Wl,-subsystem,console"]
        } else if cfg!(target_os = "macos") {
            vec!["-lSystem"]
        } else {
            vec!["-lc"]
        };
        Self {
            libs,
            asm_outputs: Vec::new(),
        }
    }

    pub fn add_asm(&mut self, path: String, body: String) {
        self.asm_outputs.push(AsmOutput { path, body });
    }

    pub fn compile(&self, ir: &str, out: &str) {
        std::fs::write("out.ll", ir).unwrap();
        Command::new("clang")
            .args(["-c", "out.ll", "-o", "out.o"])
            .status()
            .expect("IR compile failed");
        let obj_files = self.compile_asm();
        self.link(&obj_files, out);
    }

    fn compile_asm(&self) -> Vec<String> {
        let mut obj_files = vec!["out.o".to_string()];
        for asm in &self.asm_outputs {
            std::fs::write(&asm.path, &asm.body).unwrap();
            let obj = asm.path.replace(".s", ".o");
            Command::new("clang")
                .args(["-c", &asm.path, "-o", &obj])
                .status()
                .expect("ASM compile failed");
            obj_files.push(obj);
        }
        obj_files
    }

    fn link(&self, obj_files: &[String], out: &str) {
        let mut cmd = Command::new("clang");
        for obj in obj_files {
            cmd.arg(obj);
        }
        for lib in &self.libs {
            cmd.arg(lib);
        }
        cmd.arg("-o").arg(out);
        cmd.status().expect("Link failed");
    }
}
