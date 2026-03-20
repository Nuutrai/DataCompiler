use std::env::args;
use std::process::Command;

pub struct AsmOutput {
    pub path: String,
    pub body: String,
}

pub struct Compiler {
    target: String,
    libs: Vec<&'static str>,
    linker_options: Vec<&'static str>,
    os_flags: Vec<&'static str>,
    asm_outputs: Vec<AsmOutput>,
}

impl Compiler {
    pub fn new(target: &str) -> Self {
        let mut libs = Vec::new();
        let mut os_flags = Vec::new();
        let mut linker_options = Vec::new();
        // linker_options.push("-v");
        if cfg!(target_os = "windows") {
            libs.push("-lkernel32");
            linker_options.push("-Wl,-subsystem,console");
            // libs.push("-Wl,-subsystem,console");
        } else if cfg!(target_os = "macos") {
            // libs.push("-lSystem");
            // linker_options.push("-Wl,-e,main");
            os_flags.push("-masm=intel");
        } else {
            libs.push("-lc");
        }
        Self {
            target: target.to_owned(),
            libs,
            linker_options,
            os_flags,
            asm_outputs: Vec::new(),
        }
    }

    pub fn add_asm(&mut self, path: String, body: String) {
        self.asm_outputs.push(AsmOutput { path, body });
    }

    pub fn compile(&self, ir: &str, out: &str, target: &str) {
        std::fs::write("out.ll", ir).unwrap();
        Command::new("clang")
            .args(self.os_flags.clone())
            .arg(format!("--target={target}").as_str())
            .args(["-Woverride-module", "-c", "out.ll", "-o", "out.o"])
            .status()
            .expect("IR compile failed");
        let obj_files = self.compile_asm(target);
        self.link(&obj_files, out);
    }

    fn compile_asm(&self, target: &str) -> Vec<String> {
        let mut obj_files = vec!["out.o".to_string()];
        for asm in &self.asm_outputs {
            std::fs::write(&asm.path, &asm.body).unwrap();
            let obj = asm.path.replace(".s", ".o");
            Command::new("clang")
                .arg(format!("--target={target}").as_str())
                .args(self.os_flags.clone())
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
        cmd
            .arg(format!("--target={}", self.target).as_str())
            .args(self.linker_options.clone())
            .arg("-o")
            .arg(out);
        cmd.status().expect("Link failed");
    }
}
