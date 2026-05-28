#[cfg(test)]
mod tests {
    use musubu_driver::compile;
    use musubu_engine::MusubuEngine;
    use std::fs;

    #[test]
    fn test_full() {
        let content = fs::read_to_string("../../test.msb").unwrap();
        let mut engine = MusubuEngine::new();

        assert!(compile(&mut engine, content.as_str()))
    }
}
