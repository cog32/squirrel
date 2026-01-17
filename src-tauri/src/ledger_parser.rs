use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
  pub line: usize,
  pub column: usize,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParseResult {
  pub ok: bool,
  pub diagnostics: Vec<Diagnostic>,
}

fn diag(line: usize, column: usize, message: impl Into<String>) -> Diagnostic {
  Diagnostic {
    line,
    column,
    message: message.into(),
  }
}

fn is_blank(line: &str) -> bool {
  line.trim().is_empty()
}

fn is_directive(line: &str) -> bool {
  let trimmed = line.trim_start_matches([' ', '\t']);
  trimmed.starts_with(';')
}

fn header_datetime_re() -> Regex {
  Regex::new(
    r"^(?P<date>\d{4}-\d{2}-\d{2})(?P<time>T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:Z|[+-]\d{2}:\d{2})?)?\s+",
  )
  .expect("datetime regex")
}

fn account_re() -> Regex {
  Regex::new(r"^[A-Za-z0-9_.-]+:[A-Za-z0-9_.-]+:[A-Za-z0-9_.-]+(?:[:][A-Za-z0-9_.-]+)*$")
    .expect("account regex")
}

fn amount_re() -> Regex {
  Regex::new(r"^[+-]?\d+(?:\.\d+)?$").expect("amount regex")
}

fn commodity_re() -> Regex {
  Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("commodity regex")
}

pub fn parse_transactions(contents: &str) -> ParseResult {
  let header_re = header_datetime_re();
  let account_re = account_re();
  let amount_re = amount_re();
  let commodity_re = commodity_re();

  let mut diagnostics: Vec<Diagnostic> = Vec::new();
  let mut in_txn = false;
  let mut saw_posting = false;

  for (idx, raw_line) in contents.lines().enumerate() {
    let line_no = idx + 1;
    let line = raw_line.trim_end_matches('\r');

    if is_blank(line) {
      if in_txn {
        in_txn = false;
        saw_posting = false;
      }
      continue;
    }

    if is_directive(line) {
      continue;
    }

    if line.starts_with("    ") {
      if !in_txn {
        diagnostics.push(diag(line_no, 0, "posting without transaction header"));
        continue;
      }

      let after_indent = &line[4..];
      let mut parts = after_indent.split_whitespace();
      let account = parts.next();
      let amount = parts.next();
      let commodity = parts.next();

      if account.is_none() {
        diagnostics.push(diag(line_no, 4, "missing account"));
        continue;
      }

      let account = account.unwrap();
      if !account_re.is_match(account) {
        diagnostics.push(diag(
          line_no,
          4,
          format!("invalid account path: {account}"),
        ));
      }

      if amount.is_none() {
        diagnostics.push(diag(line_no, 4 + account.len() + 1, "missing amount"));
        continue;
      }

      let amount = amount.unwrap();
      if !amount_re.is_match(amount) {
        diagnostics.push(diag(
          line_no,
          4 + account.len() + 1,
          format!("invalid amount: {amount}"),
        ));
      }

      if commodity.is_none() {
        diagnostics.push(diag(
          line_no,
          4 + account.len() + 1 + amount.len() + 1,
          "missing commodity",
        ));
        continue;
      }

      let commodity = commodity.unwrap();
      if !commodity_re.is_match(commodity) {
        diagnostics.push(diag(
          line_no,
          4 + account.len() + 1 + amount.len() + 1,
          format!("invalid commodity: {commodity}"),
        ));
      }

      saw_posting = true;
      continue;
    }

    // Header line
    if let Some(m) = header_re.find(line) {
      let rest = &line[m.end()..];
      if rest.trim().is_empty() {
        diagnostics.push(diag(line_no, m.end(), "missing transaction details"));
      }
      if !line.contains(';') {
        diagnostics.push(diag(line_no, 0, "missing meta comment (expected ';')"));
      }

      in_txn = true;
      saw_posting = false;
    } else {
      diagnostics.push(diag(
        line_no,
        0,
        "invalid line: expected transaction header, posting, or directive",
      ));
    }
  }

  // If a file ends while still in a txn, enforce at least one posting.
  if in_txn && !saw_posting {
    diagnostics.push(diag(
      contents.lines().count().max(1),
      0,
      "transaction missing postings",
    ));
  }

  ParseResult {
    ok: diagnostics.is_empty(),
    diagnostics,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_valid_fixture() {
    let input = r#"2026-01-15 * \"Binance\" \"Buy SOL\" ; txn:01J2N9R9, src:binance:order:999
    assets:exchange:binance:sol    10.000000 SOL {{ 230.00 USD, fee:0.10 USD, fee_to:expenses:fees:trading, venue:binance, note:\"maker fee\" }}
    assets:cash:usd              -230.10 USD
"#;

    let result = parse_transactions(input);
    assert!(result.ok, "expected ok, got diagnostics: {:?}", result.diagnostics);
  }

  #[test]
  fn rejects_posting_without_amount() {
    let input = r#"2026-01-15 * \"Binance\" \"Buy SOL\" ; txn:01J2N9R9
    assets:exchange:binance:sol
"#;

    let result = parse_transactions(input);
    assert!(!result.ok);
    assert!(result
      .diagnostics
      .iter()
      .any(|d| d.message.contains("missing amount")));
  }
}
