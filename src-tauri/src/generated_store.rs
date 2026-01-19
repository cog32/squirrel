use crate::generated_ledger::rotate_ledger_if_needed;
use crate::ledger_parser::{parse_transactions, ParseResult, Posting, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GeneratedIndex {
  txn_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SourceRegistry {
  paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportStats {
  pub imported: usize,
  pub skipped_duplicates: usize,
  pub archived: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPostingInput {
  pub account: String,
  pub amount: String,
  pub commodity: String,
  pub remainder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualTransactionInput {
  /// Either `YYYY-MM-DD` or a full datetime supported by the ledger grammar.
  pub datetime: String,
  pub status: Option<char>,
  pub payee: String,
  pub narration: String,
  pub postings: Vec<ManualPostingInput>,
}

fn generated_ledger_path(base_dir: &Path) -> PathBuf {
  base_dir.join("ledger.transactions")
}

fn generated_archive_dir(base_dir: &Path) -> PathBuf {
  base_dir.join("archive")
}

fn generated_archive_path(base_dir: &Path, yyyymm: &str) -> PathBuf {
  generated_archive_dir(base_dir).join(format!("ledger-{yyyymm}.transactions"))
}

fn index_path(base_dir: &Path) -> PathBuf {
  base_dir.join("index.json")
}

fn sources_path(base_dir: &Path) -> PathBuf {
  base_dir.join("sources.json")
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> io::Result<T> {
  if !path.exists() {
    return Ok(T::default());
  }
  let contents = fs::read_to_string(path)?;
  Ok(serde_json::from_str(&contents).unwrap_or_default())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
  let contents = serde_json::to_string_pretty(value).expect("json serialize");
  fs::write(path, contents)
}

fn quote(s: &str) -> String {
  let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
  format!("\"{escaped}\"")
}

fn extract_txn_id(meta: &str) -> Option<String> {
  // Token ends at comma or whitespace.
  let pos = meta.find("txn:")?;
  let after = &meta[pos + 4..];
  let end = after
    .find(|c: char| c == ',' || c.is_whitespace())
    .unwrap_or(after.len());
  let id = after[..end].trim();
  if id.is_empty() {
    None
  } else {
    Some(id.to_string())
  }
}

fn ensure_txn_id(meta: Option<&str>) -> (String, String) {
  if let Some(m) = meta {
    if let Some(id) = extract_txn_id(m) {
      return (id, m.to_string());
    }
  }

  let id = generate_txn_id();
  let updated = match meta {
    Some(m) if !m.trim().is_empty() => format!("{}, txn:{id}", m.trim()),
    _ => format!("txn:{id}"),
  };
  (id, updated)
}

fn generate_txn_id() -> String {
  let nanos = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_nanos();
  let pid = std::process::id();
  format!("gen-{pid:x}-{nanos:x}")
}

fn yyyymm_from_datetime(datetime: &str) -> Option<String> {
  if datetime.len() < 7 {
    return None;
  }
  let year = &datetime[0..4];
  let month = &datetime[5..7];
  if year.chars().all(|c| c.is_ascii_digit()) && month.chars().all(|c| c.is_ascii_digit()) {
    Some(format!("{year}{month}"))
  } else {
    None
  }
}

fn posting_to_text(posting: &Posting) -> String {
  let mut out = format!(
    "    {} {} {}",
    posting.account, posting.amount_text, posting.commodity
  );
  if let Some(rem) = posting.remainder.as_ref().and_then(|s| {
    let trimmed = s.trim();
    if trimmed.is_empty() { None } else { Some(trimmed) }
  }) {
    out.push(' ');
    out.push_str(rem);
  }
  out
}

fn transaction_to_text(txn: &Transaction, forced_meta: &str) -> String {
  let mut header = txn.datetime.clone();
  header.push(' ');

  if let Some(status) = txn.status {
    header.push(status);
    header.push(' ');
  }

  if let Some(payee) = txn.payee.as_deref() {
    header.push_str(&quote(payee));
    header.push(' ');
  }
  if let Some(narration) = txn.narration.as_deref() {
    header.push_str(&quote(narration));
    header.push(' ');
  }

  header.push_str("; ");
  header.push_str(forced_meta);

  let mut lines = Vec::with_capacity(1 + txn.postings.len() + 1);
  lines.push(header);
  for posting in &txn.postings {
    lines.push(posting_to_text(posting));
  }
  lines.push(String::new());
  lines.join("\n")
}

fn manual_to_transaction(input: &ManualTransactionInput) -> Transaction {
  Transaction {
    date: input.datetime.get(0..10).unwrap_or(&input.datetime).to_string(),
    datetime: input.datetime.clone(),
    status: input.status,
    payee: Some(input.payee.clone()),
    narration: Some(input.narration.clone()),
    meta: None,
    postings: input
      .postings
      .iter()
      .map(|p| Posting {
        account: p.account.clone(),
        amount: p.amount.parse().unwrap_or(0.0),
        amount_text: p.amount.clone(),
        commodity: p.commodity.clone(),
        remainder: p.remainder.clone(),
      })
      .collect(),
  }
}

fn append_text(path: &Path, text: &str) -> io::Result<()> {
  if !path.exists() {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    return Ok(());
  }

  let mut existing = fs::read_to_string(path)?;
  existing = normalize_blank_lines(&existing);
  if !existing.is_empty() {
    // Ensure a blank line between transactions for readability.
    if existing.ends_with("\n\n") {
      // ok
    } else if existing.ends_with('\n') {
      existing.push('\n');
    } else {
      existing.push_str("\n\n");
    }
  }
  existing.push_str(text);
  fs::write(path, existing)
}

fn looks_like_header(line: &str) -> bool {
  // Fast check for `YYYY-MM-DD` prefix.
  let b = line.as_bytes();
  if b.len() < 10 {
    return false;
  }
  b[0].is_ascii_digit()
    && b[1].is_ascii_digit()
    && b[2].is_ascii_digit()
    && b[3].is_ascii_digit()
    && b[4] == b'-'
    && b[5].is_ascii_digit()
    && b[6].is_ascii_digit()
    && b[7] == b'-'
    && b[8].is_ascii_digit()
    && b[9].is_ascii_digit()
}

fn normalize_blank_lines(contents: &str) -> String {
  let mut out: Vec<String> = Vec::new();
  let mut prev_nonblank_was_posting = false;

  for raw_line in contents.split('\n') {
    let is_blank = raw_line.trim().is_empty();
    let is_posting = raw_line.starts_with("    ");
    let is_header = looks_like_header(raw_line);

    if is_header && prev_nonblank_was_posting {
      if out.last().is_some_and(|l| !l.trim().is_empty()) {
        out.push(String::new());
      }
    }

    out.push(raw_line.to_string());

    if is_blank {
      prev_nonblank_was_posting = false;
    } else {
      prev_nonblank_was_posting = is_posting;
    }
  }

  // Avoid growing the file with extra trailing blank lines.
  while out.last().is_some_and(|l| l.is_empty()) && out.len() > 1 {
    out.pop();
  }

  out.join("\n")
}

pub fn import_source_files(
  base_dir: &Path,
  now_yyyymm: &str,
  paths: &[String],
) -> Result<ImportStats, String> {
  fs::create_dir_all(base_dir).map_err(|e| e.to_string())?;
  rotate_ledger_if_needed(base_dir, now_yyyymm).map_err(|e| e.to_string())?;

  let mut index: GeneratedIndex = read_json(&index_path(base_dir)).map_err(|e| e.to_string())?;
  let mut sources: SourceRegistry = read_json(&sources_path(base_dir)).map_err(|e| e.to_string())?;

  let mut imported = 0usize;
  let mut skipped_duplicates = 0usize;
  let mut archived = 0usize;

  for p in paths {
    if !sources.paths.contains(p) {
      sources.paths.push(p.clone());
    }

    let contents =
      fs::read_to_string(p).map_err(|e| format!("failed to read source file {p}: {e}"))?;
    let result: ParseResult = parse_transactions(&contents);
    if result.transactions.is_empty() {
      continue;
    }

    for txn in result.transactions {
      if txn.postings.is_empty() {
        continue;
      }

      let (id, meta) = ensure_txn_id(txn.meta.as_deref());
      if index.txn_ids.contains(&id) {
        skipped_duplicates += 1;
        continue;
      }

      let yyyymm = yyyymm_from_datetime(&txn.datetime).unwrap_or_else(|| now_yyyymm.to_string());
      let dest = if yyyymm == now_yyyymm {
        generated_ledger_path(base_dir)
      } else {
        archived += 1;
        generated_archive_path(base_dir, &yyyymm)
      };

      let text = transaction_to_text(&txn, &meta);
      append_text(&dest, &text).map_err(|e| e.to_string())?;
      index.txn_ids.insert(id);
      imported += 1;
    }
  }

  sources.paths.sort();
  write_json(&sources_path(base_dir), &sources).map_err(|e| e.to_string())?;
  write_json(&index_path(base_dir), &index).map_err(|e| e.to_string())?;

  Ok(ImportStats {
    imported,
    skipped_duplicates,
    archived,
  })
}

pub fn add_manual_transaction(
  base_dir: &Path,
  now_yyyymm: &str,
  input: &ManualTransactionInput,
) -> Result<String, String> {
  fs::create_dir_all(base_dir).map_err(|e| e.to_string())?;

  let mut index: GeneratedIndex = read_json(&index_path(base_dir)).map_err(|e| e.to_string())?;
  let txn = manual_to_transaction(input);

  let (id, meta) = ensure_txn_id(None);
  if index.txn_ids.contains(&id) {
    return Err("generated txn id collided; retry".to_string());
  }

  let yyyymm = yyyymm_from_datetime(&txn.datetime).unwrap_or_else(|| now_yyyymm.to_string());
  let dest = if yyyymm == now_yyyymm {
    generated_ledger_path(base_dir)
  } else {
    generated_archive_path(base_dir, &yyyymm)
  };

  let text = transaction_to_text(&txn, &meta);
  append_text(&dest, &text).map_err(|e| e.to_string())?;
  index.txn_ids.insert(id.clone());
  write_json(&index_path(base_dir), &index).map_err(|e| e.to_string())?;

  Ok(id)
}

pub fn add_account_declaration(
  base_dir: &Path,
  account_name: &str,
  currency: Option<&str>,
  opening_balance: Option<&str>,
) -> Result<(), String> {
  fs::create_dir_all(base_dir).map_err(|e| e.to_string())?;

  let dest = generated_ledger_path(base_dir);

  // Build the account declaration line
  let mut text = format!("account {}", account_name);
  if let Some(curr) = currency {
    text.push(' ');
    text.push_str(curr);
  }
  text.push('\n');

  // Add opening balance if provided
  if let Some(balance) = opening_balance {
    let curr = currency.unwrap_or("USD");
    text.push_str(&format!("    opening {} {}\n", balance, curr));
  }

  append_text(&dest, &text).map_err(|e| e.to_string())?;

  Ok(())
}

pub fn load_active_ledger(base_dir: &Path) -> Result<ParseResult, String> {
  let ledger = generated_ledger_path(base_dir);
  if !ledger.exists() {
    return Ok(ParseResult {
      ok: true,
      diagnostics: Vec::new(),
      transactions: Vec::new(),
      balances: Vec::new(),
    });
  }

  let contents = fs::read_to_string(&ledger).map_err(|e| e.to_string())?;
  let normalized = normalize_blank_lines(&contents);
  if normalized != contents {
    fs::write(&ledger, &normalized).map_err(|e| e.to_string())?;
  }
  Ok(parse_transactions(&normalized))
}
