use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use bumpalo::Bump;
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

/// Stores extracted parameter information from a parsed PHP method.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// The parameter name including the `$` prefix (e.g. "$text").
    pub name: String,
    /// Whether this parameter is required (no default value and not variadic).
    pub is_required: bool,
    /// Optional type hint string (e.g. "string", "int", "?Foo").
    pub type_hint: Option<String>,
    /// Whether this parameter is variadic (has `...`).
    pub is_variadic: bool,
    /// Whether this parameter is passed by reference (has `&`).
    pub is_reference: bool,
}

/// Stores extracted method information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// The method name (e.g. "updateText").
    pub name: String,
    /// The parameters of the method.
    pub parameters: Vec<ParameterInfo>,
    /// Optional return type hint string (e.g. "void", "string", "?int").
    pub return_type: Option<String>,
    /// Whether the method is static.
    pub is_static: bool,
}

/// Stores extracted property information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// The property name WITHOUT the `$` prefix (e.g. "name", "age").
    /// This matches PHP access syntax: `$this->name` not `$this->$name`.
    pub name: String,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
    /// Whether the property is static.
    pub is_static: bool,
}

/// Stores extracted constant information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct ConstantInfo {
    /// The constant name (e.g. "MAX_SIZE", "STATUS_ACTIVE").
    pub name: String,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
}

/// Describes the access operator that triggered completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Completion triggered after `->` (instance access).
    Arrow,
    /// Completion triggered after `::` (static access).
    DoubleColon,
    /// No specific access operator detected (e.g. inside class body).
    Other,
}

/// Stores extracted class information from a parsed PHP file.
/// All data is owned so we don't depend on the parser's arena lifetime.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// The name of the class (e.g. "User").
    pub name: String,
    /// The methods defined directly in this class.
    pub methods: Vec<MethodInfo>,
    /// The properties defined directly in this class.
    pub properties: Vec<PropertyInfo>,
    /// The constants defined directly in this class.
    pub constants: Vec<ConstantInfo>,
    /// Byte offset where the class body starts (left brace).
    pub start_offset: u32,
    /// Byte offset where the class body ends (right brace).
    pub end_offset: u32,
}

pub struct Backend {
    name: String,
    version: String,
    open_files: Arc<Mutex<HashMap<String, String>>>,
    /// Maps a file URI to a list of ClassInfo extracted from that file.
    ast_map: Arc<Mutex<HashMap<String, Vec<ClassInfo>>>>,
    client: Option<Client>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: Some(client),
        }
    }

    pub fn new_test() -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: None,
        }
    }

    fn get_word_at_position(&self, content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();

        if position.character as usize > chars.len() {
            return None;
        }

        let pos = position.character as usize;

        // Find word boundaries
        let mut start = pos;
        let mut end = pos;

        // Move start backward to word boundary
        while start > 0 && chars[start - 1].is_alphanumeric() {
            start -= 1;
        }

        // Move end forward to word boundary
        while end < chars.len() && chars[end].is_alphanumeric() {
            end += 1;
        }

        if start < end {
            Some(chars[start..end].iter().collect())
        } else {
            None
        }
    }

    /// Extract a string representation of a type hint from the AST.
    fn extract_hint_string(hint: &Hint) -> String {
        match hint {
            Hint::Identifier(ident) => ident.value().to_string(),
            Hint::Nullable(nullable) => {
                format!("?{}", Self::extract_hint_string(nullable.hint))
            }
            Hint::Union(union) => {
                let left = Self::extract_hint_string(union.left);
                let right = Self::extract_hint_string(union.right);
                format!("{}|{}", left, right)
            }
            Hint::Intersection(intersection) => {
                let left = Self::extract_hint_string(intersection.left);
                let right = Self::extract_hint_string(intersection.right);
                format!("{}&{}", left, right)
            }
            Hint::Void(ident)
            | Hint::Never(ident)
            | Hint::Float(ident)
            | Hint::Bool(ident)
            | Hint::Integer(ident)
            | Hint::String(ident)
            | Hint::Object(ident)
            | Hint::Mixed(ident)
            | Hint::Iterable(ident) => ident.value.to_string(),
            Hint::Null(keyword)
            | Hint::True(keyword)
            | Hint::False(keyword)
            | Hint::Array(keyword)
            | Hint::Callable(keyword)
            | Hint::Static(keyword)
            | Hint::Self_(keyword)
            | Hint::Parent(keyword) => keyword.value.to_string(),
            Hint::Parenthesized(paren) => {
                format!("({})", Self::extract_hint_string(paren.hint))
            }
        }
    }

    /// Extract parameter information from a method's parameter list.
    fn extract_parameters(parameter_list: &FunctionLikeParameterList) -> Vec<ParameterInfo> {
        parameter_list
            .parameters
            .iter()
            .map(|param| {
                let name = param.variable.name.to_string();
                let is_variadic = param.ellipsis.is_some();
                let is_reference = param.ampersand.is_some();
                let has_default = param.default_value.is_some();
                let is_required = !has_default && !is_variadic;

                let type_hint = param.hint.as_ref().map(|h| Self::extract_hint_string(h));

                ParameterInfo {
                    name,
                    is_required,
                    type_hint,
                    is_variadic,
                    is_reference,
                }
            })
            .collect()
    }

    /// Extract property information from a class member Property node.
    fn extract_property_info(property: &Property) -> Vec<PropertyInfo> {
        let is_static = property.modifiers().iter().any(|m| m.is_static());

        let type_hint = property.hint().map(|h| Self::extract_hint_string(h));

        property
            .variables()
            .iter()
            .map(|var| {
                let raw_name = var.name.to_string();
                // Strip the leading `$` for property names since PHP access
                // syntax is `$this->name` not `$this->$name`.
                let name = if let Some(stripped) = raw_name.strip_prefix('$') {
                    stripped.to_string()
                } else {
                    raw_name
                };

                PropertyInfo {
                    name,
                    type_hint: type_hint.clone(),
                    is_static,
                }
            })
            .collect()
    }

    /// Parse PHP source text and extract class information.
    /// Returns a Vec of ClassInfo for all classes found in the file.
    pub fn parse_php(&self, content: &str) -> Vec<ClassInfo> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let mut classes = Vec::new();
        Self::extract_classes_from_statements(program.statements.iter(), &mut classes);
        classes
    }

    /// Recursively walk statements and extract class information.
    /// This handles classes at the top level as well as classes nested
    /// inside namespace declarations.
    fn extract_classes_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        classes: &mut Vec<ClassInfo>,
    ) {
        for statement in statements {
            match statement {
                Statement::Class(class) => {
                    let class_name = class.name.value.to_string();

                    let mut methods = Vec::new();
                    let mut properties = Vec::new();
                    let mut constants = Vec::new();

                    for member in class.members.iter() {
                        match member {
                            ClassLikeMember::Method(method) => {
                                let name = method.name.value.to_string();
                                let parameters = Self::extract_parameters(&method.parameter_list);
                                let return_type = method
                                    .return_type_hint
                                    .as_ref()
                                    .map(|rth| Self::extract_hint_string(&rth.hint));
                                let is_static = method.modifiers.iter().any(|m| m.is_static());

                                methods.push(MethodInfo {
                                    name,
                                    parameters,
                                    return_type,
                                    is_static,
                                });
                            }
                            ClassLikeMember::Property(property) => {
                                let mut prop_infos = Self::extract_property_info(property);
                                properties.append(&mut prop_infos);
                            }
                            ClassLikeMember::Constant(constant) => {
                                let type_hint =
                                    constant.hint.as_ref().map(|h| Self::extract_hint_string(h));
                                for item in constant.items.iter() {
                                    constants.push(ConstantInfo {
                                        name: item.name.value.to_string(),
                                        type_hint: type_hint.clone(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }

                    let start_offset = class.left_brace.start.offset;
                    let end_offset = class.right_brace.end.offset;

                    classes.push(ClassInfo {
                        name: class_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                    });
                }
                Statement::Namespace(namespace) => {
                    Self::extract_classes_from_statements(namespace.statements().iter(), classes);
                }
                _ => {}
            }
        }
    }

    /// Update the ast_map for a given file URI by parsing its content.
    fn update_ast(&self, uri: &str, content: &str) {
        let classes = self.parse_php(content);
        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri.to_string(), classes);
        }
    }

    /// Convert an LSP Position (line, character) to a byte offset in content.
    fn position_to_offset(content: &str, position: Position) -> Option<u32> {
        let mut offset: u32 = 0;
        for (i, line) in content.lines().enumerate() {
            if i == position.line as usize {
                let char_offset = position.character as usize;
                // Convert character offset (UTF-16 code units in LSP) to byte offset.
                // For simplicity, treat characters as single-byte (ASCII).
                // This is sufficient for most PHP code.
                let byte_col = line
                    .char_indices()
                    .nth(char_offset)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
                return Some(offset + byte_col as u32);
            }
            // +1 for the newline character
            offset += line.len() as u32 + 1;
        }
        // If the position is past the last line, return end of content
        Some(content.len() as u32)
    }

    /// Find which class the cursor (byte offset) is inside.
    fn find_class_at_offset(classes: &[ClassInfo], offset: u32) -> Option<&ClassInfo> {
        classes
            .iter()
            .find(|c| offset >= c.start_offset && offset <= c.end_offset)
    }

    /// Detect the access operator before the cursor position by scanning
    /// backwards past any partial identifier the user may have typed.
    pub fn detect_access_kind(content: &str, position: Position) -> AccessKind {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return AccessKind::Other;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards past any identifier characters the user may have typed
        let mut i = col;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        // Now check for `->` or `::`
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            AccessKind::Arrow
        } else if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            AccessKind::DoubleColon
        } else {
            AccessKind::Other
        }
    }

    /// Build the label showing the full method signature.
    ///
    /// Example: `regularCode(string $text, $frogs = false): string`
    fn build_method_label(method: &MethodInfo) -> String {
        let params: Vec<String> = method
            .parameters
            .iter()
            .map(|p| {
                let mut parts = Vec::new();
                if let Some(ref th) = p.type_hint {
                    parts.push(th.clone());
                }
                if p.is_reference {
                    parts.push(format!("&{}", p.name));
                } else if p.is_variadic {
                    parts.push(format!("...{}", p.name));
                } else {
                    parts.push(p.name.clone());
                }
                let param_str = parts.join(" ");
                if !p.is_required && !p.is_variadic {
                    format!("{} = ...", param_str)
                } else {
                    param_str
                }
            })
            .collect();

        let ret = method
            .return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        format!("{}({}){}", method.name, params.join(", "), ret)
    }

    /// Public helper for tests: get the ast_map for a given URI.
    pub fn get_classes_for_uri(&self, uri: &str) -> Option<Vec<ClassInfo>> {
        if let Ok(map) = self.ast_map.lock() {
            map.get(uri).cloned()
        } else {
            None
        }
    }

    async fn log(&self, typ: MessageType, message: String) {
        if let Some(client) = &self.client {
            client.log_message(typ, message).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "$".to_string(),
                        ">".to_string(),
                        ":".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: self.name.clone(),
                version: Some(self.version.clone()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.log(MessageType::INFO, "PHPantomLSP initialized!".to_string())
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let uri = doc.uri.to_string();
        let text = doc.text;

        // Store file content
        if let Ok(mut files) = self.open_files.lock() {
            files.insert(uri.clone(), text.clone());
        }

        // Parse and update AST map
        self.update_ast(&uri, &text);

        self.log(MessageType::INFO, format!("Opened file: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Some(change) = params.content_changes.first() {
            let text = &change.text;

            // Update stored content
            if let Ok(mut files) = self.open_files.lock() {
                files.insert(uri.clone(), text.clone());
            }

            // Re-parse and update AST map
            self.update_ast(&uri, text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Ok(mut files) = self.open_files.lock() {
            files.remove(&uri);
        }

        if let Ok(mut map) = self.ast_map.lock() {
            map.remove(&uri);
        }

        self.log(MessageType::INFO, format!("Closed file: {}", uri))
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let content = if let Ok(files) = self.open_files.lock() {
            files.get(&uri).cloned()
        } else {
            None
        };

        if let Some(content) = content
            && let Some(word) = self.get_word_at_position(&content, position)
            && word == "PHPantom"
        {
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(
                    "Welcome to PHPantomLSP!".to_string(),
                )),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        // Get file content for offset calculation
        let content = if let Ok(files) = self.open_files.lock() {
            files.get(&uri).cloned()
        } else {
            None
        };

        // Get classes from ast_map
        let classes = if let Ok(map) = self.ast_map.lock() {
            map.get(&uri).cloned()
        } else {
            None
        };

        if let (Some(content), Some(classes)) = (content, classes)
            && let Some(offset) = Self::position_to_offset(&content, position)
            && let Some(class_info) = Self::find_class_at_offset(&classes, offset)
        {
            let mut items: Vec<CompletionItem> = Vec::new();
            let access_kind = Self::detect_access_kind(&content, position);

            // Add method completions (filtered by access kind)
            for method in &class_info.methods {
                let include = match access_kind {
                    AccessKind::Arrow => !method.is_static,
                    AccessKind::DoubleColon => method.is_static,
                    AccessKind::Other => true,
                };
                if !include {
                    continue;
                }

                let label = Self::build_method_label(method);

                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::METHOD),
                    detail: Some(format!("Class: {}", class_info.name)),
                    insert_text: Some(method.name.clone()),
                    filter_text: Some(method.name.clone()),
                    ..CompletionItem::default()
                });
            }

            // Add property completions (filtered by access kind)
            for property in &class_info.properties {
                let include = match access_kind {
                    AccessKind::Arrow => !property.is_static,
                    AccessKind::DoubleColon => property.is_static,
                    AccessKind::Other => true,
                };
                if !include {
                    continue;
                }

                let detail = if let Some(ref th) = property.type_hint {
                    format!("Class: {} — {}", class_info.name, th)
                } else {
                    format!("Class: {}", class_info.name)
                };

                items.push(CompletionItem {
                    label: property.name.clone(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(detail),
                    insert_text: Some(property.name.clone()),
                    ..CompletionItem::default()
                });
            }

            // Add constant completions (only for `::` or unqualified access)
            if access_kind == AccessKind::DoubleColon || access_kind == AccessKind::Other {
                for constant in &class_info.constants {
                    let detail = if let Some(ref th) = constant.type_hint {
                        format!("Class: {} — {}", class_info.name, th)
                    } else {
                        format!("Class: {}", class_info.name)
                    };

                    items.push(CompletionItem {
                        label: constant.name.clone(),
                        kind: Some(CompletionItemKind::CONSTANT),
                        detail: Some(detail),
                        insert_text: Some(constant.name.clone()),
                        filter_text: Some(constant.name.clone()),
                        ..CompletionItem::default()
                    });
                }
            }

            if !items.is_empty() {
                return Ok(Some(CompletionResponse::Array(items)));
            }
        }

        // Fallback: return the default PHPantomLSP completion item
        Ok(Some(CompletionResponse::Array(vec![CompletionItem {
            label: "PHPantomLSP".to_string(),
            kind: Some(CompletionItemKind::TEXT),
            detail: Some("PHPantomLSP completion".to_string()),
            insert_text: Some("PHPantomLSP".to_string()),
            ..CompletionItem::default()
        }])))
    }
}
