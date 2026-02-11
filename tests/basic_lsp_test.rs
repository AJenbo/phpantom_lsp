use phpantom_lsp::Backend;

#[test]
fn test_backend_name_version() {
    let backend = Backend::new();
    assert_eq!(backend.get_name(), "PHPantomLSP");
    assert_eq!(backend.get_version(), "0.1.0");
}
