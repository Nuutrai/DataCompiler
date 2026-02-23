pub struct Value {
    pub name: String,
    pub ty: String,
}

impl Value {
    pub fn new(name: &str, ty: &str) -> Self {
        Self {
            name: name.to_string(),
            ty: ty.to_string(),
        }
    }

    pub fn as_arg(&self) -> String {
        format!("{} {}", self.ty, self.name)
    }
}
