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
    header_idx: u32,
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

        self.on_change(TextDocumentItem {
            text: &params.text_document.text,
            uri,
        })
        .await
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        self.client
            .log_message(MessageType::INFO, format!("file changed! URL: {}", uri))
            .await;

        self.on_change(TextDocumentItem {
            text: &params.content_changes[0].text,
            uri,
        })
        .await
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
            CompletionItem::new_simple("REMOVE".to_string(), "Remove data".to_string()),
        ])))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let highlight_list = || -> Option<Vec<DocumentHighlight>> {
            let uri = params.text_document_position_params.text_document.uri;
            let idx = params.text_document_position_params.position.line;
            let full_uri = format!("{}:{}", uri, idx);

            let line = self.line_map.get(full_uri.as_str())?;
            let content = &line.content;
            let header_idx = &line.header_idx;

            let start = params.text_document_position_params.position.character;
            if content.chars().nth(start as usize) == Some(';') {
                return None;
            }

            let content_up_to_start = &content[0..start as usize];
            let semicolon_count = content_up_to_start.chars().filter(|&c| c == ';').count();

            let mut higlights: Vec<DocumentHighlight> = Vec::new();

            let (range, header_check) = match idx == *header_idx {
                true => ((idx + 1)..=(idx + 30), Some(idx)),
                false => (*header_idx..=*header_idx, None),
            };

            for i in range {
                let temp_uri = format!("{}:{}", uri, i);
                let temp_line = match self.line_map.get(temp_uri.as_str()) {
                    Some(line) => line,
                    None => continue,
                };

                if let Some(expected_header) = header_check {
                    if temp_line.header_idx != expected_header {
                        break;
                    }
                }

                let temp_content = &temp_line.content;

                let mut target_start = temp_content
                    .char_indices()
                    .filter(|&(_, ch)| ch == ';')
                    .map(|(i, _)| i as u32)
                    .skip(semicolon_count - 1);

                let range = Range {
                    start: Position {
                        line: i,
                        character: target_start.next()? + 1,
                    },
                    end: Position {
                        line: i,
                        character: target_start.next()?,
                    },
                };

                let highlight = DocumentHighlight {
                    range,
                    kind: Some(DocumentHighlightKind::TEXT),
                };

                higlights.push(highlight);
            }

            Some(higlights)
        }();

        Ok(highlight_list)
    }
}

struct TextDocumentItem<'a> {
    uri: String,
    text: &'a str,
}

impl Backend {
    async fn on_change<'a>(&self, params: TextDocumentItem<'a>) {
        let keywords = ["INSERT_UPDATE", "INSERT", "UPDATE", "DELETE", "REMOVE"];

        params
            .text
            .lines()
            .enumerate()
            .filter(|&(_, line)| line.contains(";") && !line.contains("#"))
            .scan(0, |header_idx, (idx, line)| {
                let line = format!("{};", line);
                if keywords
                    .iter()
                    .any(|prefix| line.trim_start().starts_with(prefix))
                {
                    *header_idx = idx as u32;
                }
                Some((idx, line, *header_idx))
            })
            .for_each(|(idx, content, header_idx)| {
                self.line_map.insert(
                    format!("{}:{}", params.uri, idx),
                    Line {
                        content,
                        header_idx,
                    },
                );
            });
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
