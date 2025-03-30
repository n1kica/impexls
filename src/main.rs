use dashmap::DashMap;
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    line_map: DashMap<String, Line>,
}

struct Line {
    content: String,
    header_idx: usize,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    ..Default::default()
                }),
                document_highlight_provider: Some(OneOf::Left(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
            .await;

        match self.client.apply_edit(WorkspaceEdit::default()).await {
            Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
            Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
            Err(err) => self.client.log_message(MessageType::ERROR, err).await,
        }

        Ok(None)
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        self.client
            .log_message(MessageType::INFO, format!("file opened! URL: {}", uri))
            .await;

        let keywords = ["INSERT_UPDATE", "INSERT", "UPDATE", "DELETE", "REMOVE"];

        params
            .text_document
            .text
            .lines()
            .filter(|line| line.contains(";"))
            .enumerate()
            .scan(0, |header_idx, (idx, line)| {
                let line = format!("{};", line);
                if keywords
                    .iter()
                    .any(|prefix| line.trim_start().starts_with(prefix))
                {
                    *header_idx = idx;
                }
                Some((idx, line, *header_idx))
            })
            .for_each(|(idx, content, header_idx)| {
                self.line_map.insert(
                    format!("{}:{}", uri, idx),
                    Line {
                        content,
                        header_idx,
                    },
                );
            });
    }

    async fn did_change(&self, _: DidChangeTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file changed!")
            .await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(vec![
            CompletionItem::new_simple(
                "INSERT_UPDATE".to_string(),
                "Insert/Update data".to_string(),
            ),
            CompletionItem::new_simple("INSERT".to_string(), "Insert data".to_string()),
            CompletionItem::new_simple("UPDATE".to_string(), "Update data".to_string()),
            CompletionItem::new_simple("DELETE".to_string(), "Delete data".to_string()),
        ])))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        self.client
            .log_message(MessageType::INFO, format!("WE ARE NOT EVEN IN"))
            .await;
        let highlight_list = || -> Option<Vec<DocumentHighlight>> {
            let uri = params.text_document_position_params.text_document.uri;
            let idx = params.text_document_position_params.position.line;
            let start = params.text_document_position_params.position.character;
            let full_uri = format!("{}:{}", uri, idx);

            let line = self.line_map.get(full_uri.as_str())?;
            let content = &line.content;
            let header_idx = &line.header_idx;

            let content_up_to_start = &content[0..start as usize];
            let semicolon_count = content_up_to_start.chars().filter(|&c| c == ';').count();

            let header_uri = format!("{}:{}", uri, header_idx);
            let header = self.line_map.get(header_uri.as_str())?;
            let header_content = &header.content;

            let mut ture_start = header_content
                .char_indices()
                .filter(|&(_, ch)| ch == ';')
                .map(|(i, _)| i)
                .skip(semicolon_count - 1);

            let range = Range {
                start: Position {
                    line: *header_idx as u32,
                    character: ture_start.next()? as u32 + 1,
                },
                end: Position {
                    line: *header_idx as u32,
                    character: ture_start.next()? as u32,
                },
            };

            let highlight = DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::TEXT), // You can adjust the kind if needed
            };

            return Some(vec![highlight]); // Return the highlight as a vector
        }();
        if let Some(highlights) = &highlight_list {
            if let Some(first_highlight) = highlights.first() {
                println!("{:?}", first_highlight);
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "HOWER: Line: {}, Start: {}, End: {}",
                            first_highlight.range.start.line,
                            first_highlight.range.start.character,
                            first_highlight.range.end.character,
                        ),
                    )
                    .await;
            }
        } else {
            self.client
                .log_message(MessageType::INFO, format!("WE DIDNT MAKE IT"))
                .await;
        }

        Ok(highlight_list) // Return None if no highlight is found
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

    let (service, socket) = LspService::new(|client| Backend {
        client,
        line_map: DashMap::new(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
