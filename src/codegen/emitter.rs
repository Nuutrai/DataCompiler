pub struct Emitter {
    output: String,
    pub globals: String,
    tmp: usize,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            globals: String::new(),
            tmp: 0,
        }
    }

    pub fn fresh(&mut self) -> String {
        let name = format!("%t{}", self.tmp);
        self.tmp += 1;
        name
    }

    pub fn tmp_counter(&mut self) -> usize {
        let n = self.tmp;
        self.tmp += 1;
        n
    }

    pub fn emit(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }

    pub fn emit_global(&mut self, line: &str) {
        self.globals.push_str(line);
        self.globals.push('\n');
    }

    pub fn finish(self) -> (String, String) {
        (self.globals, self.output)
    }

    pub fn finish_ref(&self) -> (&str, &str) {
        (&self.globals, &self.output)
    }
}
