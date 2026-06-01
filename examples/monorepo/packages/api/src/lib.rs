pub fn greeting(name: &str) -> Result<String, &'static str> {
    if name.trim().is_empty() {
        return Err("missing name");
    }

    Ok(format!("hello, {name}"))
}
