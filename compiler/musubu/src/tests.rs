#[cfg(test)]
mod tests {
    use musubu_driver::compile;
    use musubu_engine::MusubuEngine;

    #[test]
    fn test_full() {
        let content = include_str!("../../../test.msb");
        let mut engine = MusubuEngine::new();

        assert!(compile(&mut engine, content))
    }
}
