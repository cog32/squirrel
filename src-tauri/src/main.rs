#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use squirrel_covid::ledger_parser::{parse_transactions, Diagnostic, ParseResult};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ParseResponse {
  ok: bool,
  diagnostics: Vec<Diagnostic>,
}

#[tauri::command]
fn parse_transactions_file(path: String) -> Result<ParseResponse, String> {
  let contents = std::fs::read_to_string(&path).map_err(|e| format!("failed to read file: {e}"))?;
  let result: ParseResult = parse_transactions(&contents);
  Ok(ParseResponse {
    ok: result.ok,
    diagnostics: result.diagnostics,
  })
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![parse_transactions_file])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
