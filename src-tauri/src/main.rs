#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use squirrel_covid::generated_ledger::rotate_ledger_if_needed;
use squirrel_covid::generated_store::{add_account_declaration, add_manual_transaction, import_source_files, load_active_ledger, ImportStats, ManualTransactionInput};
use squirrel_covid::ledger_parser::{
  parse_transactions, AccountBalance, Diagnostic, ParseResult, Transaction,
};
use serde::Serialize;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct ParseResponse {
  ok: bool,
  diagnostics: Vec<Diagnostic>,
  transactions: Vec<Transaction>,
  balances: Vec<AccountBalance>,
}

impl From<ParseResult> for ParseResponse {
  fn from(result: ParseResult) -> Self {
    Self {
      ok: result.ok,
      diagnostics: result.diagnostics,
      transactions: result.transactions,
      balances: result.balances,
    }
  }
}

#[derive(Debug, Serialize)]
struct ImportResponse {
  stats: ImportStats,
  parse: ParseResponse,
}

fn resolve_generated_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
  if let Ok(custom) = env::var("SQUIRREL_GENERATED_DIR") {
    return Ok(PathBuf::from(custom));
  }

  let app_dir = app
    .path_resolver()
    .app_data_dir()
    .ok_or_else(|| "failed to resolve app data dir".to_string())?;
  Ok(app_dir.join("generated"))
}

#[tauri::command]
fn rotate_generated_ledger(app: tauri::AppHandle, now_yyyymm: String) -> Result<String, String> {
  let generated_dir = resolve_generated_dir(&app)?;
  rotate_ledger_if_needed(&generated_dir, &now_yyyymm).map_err(|e| format!("rotate failed: {e}"))?;
  Ok(generated_dir.display().to_string())
}

#[tauri::command]
fn load_generated_ledger(app: tauri::AppHandle, now_yyyymm: String) -> Result<ParseResponse, String> {
  let generated_dir = resolve_generated_dir(&app)?;
  rotate_ledger_if_needed(&generated_dir, &now_yyyymm).map_err(|e| format!("rotate failed: {e}"))?;
  let result = load_active_ledger(&generated_dir)?;
  Ok(result.into())
}

#[tauri::command]
fn import_generated_sources(
  app: tauri::AppHandle,
  now_yyyymm: String,
  paths: Vec<String>,
) -> Result<ImportResponse, String> {
  let generated_dir = resolve_generated_dir(&app)?;
  let stats = import_source_files(&generated_dir, &now_yyyymm, &paths)?;
  let result = load_active_ledger(&generated_dir)?;
  Ok(ImportResponse {
    stats,
    parse: result.into(),
  })
}

#[tauri::command]
fn add_manual_to_generated_ledger(
  app: tauri::AppHandle,
  now_yyyymm: String,
  input: ManualTransactionInput,
) -> Result<ParseResponse, String> {
  let generated_dir = resolve_generated_dir(&app)?;
  add_manual_transaction(&generated_dir, &now_yyyymm, &input)?;
  let result = load_active_ledger(&generated_dir)?;
  Ok(result.into())
}

#[tauri::command]
fn parse_transactions_file(path: String) -> Result<ParseResponse, String> {
  let contents = std::fs::read_to_string(&path).map_err(|e| format!("failed to read file: {e}"))?;
  let result: ParseResult = parse_transactions(&contents);
  Ok(result.into())
}

#[tauri::command]
fn add_account_to_generated_ledger(
  app: tauri::AppHandle,
  account_name: String,
  currency: Option<String>,
  opening_balance: Option<String>,
) -> Result<ParseResponse, String> {
  let generated_dir = resolve_generated_dir(&app)?;
  add_account_declaration(&generated_dir, &account_name, currency.as_deref(), opening_balance.as_deref())?;
  let result = load_active_ledger(&generated_dir)?;
  Ok(result.into())
}

fn main() {
  tauri::Builder::default()
    .setup(|app| {
      // E2E-only: allow seeding generated ledger without UI interactions.
      if let Ok(paths) = env::var("SQUIRREL_E2E_IMPORT_PATHS") {
        let now_yyyymm = env::var("SQUIRREL_E2E_NOW_YYYYMM").unwrap_or_else(|_| "000000".to_string());
        let generated_dir = resolve_generated_dir(&app.handle())?;
        let list: Vec<String> = paths
          .split(';')
          .map(|s| s.trim())
          .filter(|s| !s.is_empty())
          .map(|s| s.to_string())
          .collect();
        if !list.is_empty() {
          import_source_files(&generated_dir, &now_yyyymm, &list)?;
        }
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      parse_transactions_file,
      rotate_generated_ledger,
      load_generated_ledger,
      import_generated_sources,
      add_manual_to_generated_ledger,
      add_account_to_generated_ledger
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
