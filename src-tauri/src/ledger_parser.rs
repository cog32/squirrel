use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
  pub line: usize,
  pub column: usize,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Posting {
  pub account: String,
  pub amount: f64,
  pub amount_text: String,
  pub commodity: String,
  pub remainder: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Transaction {
  pub date: String,
  pub datetime: String,
  pub status: Option<char>,
  pub payee: Option<String>,
  pub narration: Option<String>,
  pub meta: Option<String>,
  pub postings: Vec<Posting>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CommodityAmount {
  pub commodity: String,
  pub amount: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AccountBalance {
  pub account: String,
  pub totals: Vec<CommodityAmount>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ParseResult {
  pub ok: bool,
  pub diagnostics: Vec<Diagnostic>,
  pub transactions: Vec<Transaction>,
  pub balances: Vec<AccountBalance>,
}

#[derive(Debug, Clone, PartialEq)]
struct AccountDeclaration {
  account: String,
  default_commodity: Option<String>,
  opening: Option<CommodityAmount>,
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
  // Allow 2+ segments (e.g. `expenses:unknown`) for usability.
  Regex::new(r"^[A-Za-z0-9_.-]+:[A-Za-z0-9_.-]+(?:[:][A-Za-z0-9_.-]+)*$")
    .expect("account regex")
}

fn amount_re() -> Regex {
  Regex::new(r"^[+-]?\d+(?:\.\d+)?$").expect("amount regex")
}

fn commodity_re() -> Regex {
  Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("commodity regex")
}

fn take_token(input: &str) -> Option<(String, &str)> {
  let trimmed = input.trim_start_matches([' ', '\t']);
  if trimmed.is_empty() {
    return None;
  }

  if trimmed.starts_with('"') {
    let bytes = trimmed.as_bytes();
    let mut i = 1;
    let mut out = String::new();
    while i < bytes.len() {
      match bytes[i] {
        b'\\' if i + 1 < bytes.len() => {
          let next = bytes[i + 1];
          out.push(next as char);
          i += 2;
        }
        b'"' => {
          let rest = &trimmed[i + 1..];
          return Some((out, rest));
        }
        b => {
          out.push(b as char);
          i += 1;
        }
      }
    }

    // Unterminated quote; treat the entire thing as a single token (minus the opening quote).
    return Some((trimmed[1..].to_string(), ""));
  }

  let end = trimmed
    .find(|c: char| c == ' ' || c == '\t')
    .unwrap_or(trimmed.len());
  Some((trimmed[..end].to_string(), &trimmed[end..]))
}

fn parse_header_fields(header_after_datetime: &str) -> (Option<char>, Option<String>, Option<String>) {
  let mut rest = header_after_datetime;
  let mut tokens: Vec<String> = Vec::new();
  while let Some((tok, next)) = take_token(rest) {
    tokens.push(tok);
    rest = next;
  }

  let mut idx = 0;
  let status = tokens.get(0).and_then(|t| match t.as_str() {
    "*" => {
      idx = 1;
      Some('*')
    }
    "!" => {
      idx = 1;
      Some('!')
    }
    _ => None,
  });

  let payee = tokens.get(idx).cloned();
  let narration = match tokens.get(idx + 1..) {
    Some([]) | None => None,
    Some([only]) => Some(only.clone()),
    Some(many) => Some(many.join(" ")),
  };

  (status, payee, narration)
}

fn flush_transaction(
  current: &mut Option<(usize, Transaction)>,
  diagnostics: &mut Vec<Diagnostic>,
  transactions: &mut Vec<Transaction>,
) {
  if let Some((header_line, txn)) = current.take() {
    if txn.postings.is_empty() {
      diagnostics.push(diag(header_line, 0, "transaction missing postings"));
    }
    transactions.push(txn);
  }
}

pub fn parse_transactions(contents: &str) -> ParseResult {
  let header_re = header_datetime_re();
  let account_re = account_re();
  let amount_re = amount_re();
  let commodity_re = commodity_re();

  let mut diagnostics: Vec<Diagnostic> = Vec::new();
  let mut current: Option<(usize, Transaction)> = None;
  let mut current_account: Option<(usize, AccountDeclaration)> = None;
  let mut transactions: Vec<Transaction> = Vec::new();
  let mut account_declarations: Vec<AccountDeclaration> = Vec::new();

  for (idx, raw_line) in contents.lines().enumerate() {
    let line_no = idx + 1;
    let line = raw_line.trim_end_matches('\r');

    if is_blank(line) {
      flush_transaction(&mut current, &mut diagnostics, &mut transactions);
      if let Some((_, decl)) = current_account.take() {
        account_declarations.push(decl);
      }
      continue;
    }

    if is_directive(line) {
      continue;
    }

    if line == "account" || line.starts_with("account ") || line.starts_with("account\t") {
      flush_transaction(&mut current, &mut diagnostics, &mut transactions);
      if let Some((_, decl)) = current_account.take() {
        account_declarations.push(decl);
      }

      let before_meta = line.split_once(';').map(|(l, _)| l).unwrap_or(line);
      let mut parts = before_meta.split_whitespace();
      let kw = parts.next().unwrap_or_default();
      if kw != "account" {
        diagnostics.push(diag(
          line_no,
          0,
          "invalid line: expected transaction header, posting, or directive",
        ));
        continue;
      }
      let Some(account) = parts.next() else {
        diagnostics.push(diag(line_no, 0, "account declaration missing account path"));
        continue;
      };

      if !account_re.is_match(account) {
        diagnostics.push(diag(
          line_no,
          0,
          format!("invalid account path: {account}"),
        ));
      }

      let default_commodity = parts.next().map(|s| s.to_string());
      if let Some(c) = default_commodity.as_deref() {
        if !commodity_re.is_match(c) {
          diagnostics.push(diag(line_no, 0, format!("invalid commodity: {c}")));
        }
      }

      if parts.next().is_some() {
        diagnostics.push(diag(
          line_no,
          0,
          "unexpected extra tokens in account declaration (expected: account <path> [COMMODITY])",
        ));
      }

      current_account = Some((
        line_no,
        AccountDeclaration {
          account: account.to_string(),
          default_commodity,
          opening: None,
        },
      ));
      continue;
    }

    if line.starts_with("    ") {
      let Some((_, txn)) = current.as_mut() else {
        let Some((_, decl)) = current_account.as_mut() else {
          diagnostics.push(diag(line_no, 0, "unexpected indented line"));
          continue;
        };

        let after_indent = &line[4..];
        let before_meta = after_indent
          .split_once(';')
          .map(|(l, _)| l)
          .unwrap_or(after_indent);
        let mut parts = before_meta.split_whitespace();
        let Some(kind) = parts.next() else {
          diagnostics.push(diag(line_no, 4, "invalid account declaration line"));
          continue;
        };

        match kind {
          "opening" => {
            let Some(amount_text) = parts.next() else {
              diagnostics.push(diag(line_no, 4, "opening missing amount"));
              continue;
            };

            if !amount_re.is_match(amount_text) {
              diagnostics.push(diag(line_no, 4, format!("invalid amount: {amount_text}")));
            }

            let commodity = match parts.next() {
              Some(c) => Some(c.to_string()),
              None => decl.default_commodity.clone(),
            };

            let Some(commodity) = commodity else {
              diagnostics.push(diag(
                line_no,
                4,
                "opening missing commodity and account has no default commodity",
              ));
              continue;
            };

            if !commodity_re.is_match(&commodity) {
              diagnostics.push(diag(line_no, 4, format!("invalid commodity: {commodity}")));
            }

            if parts.next().is_some() {
              diagnostics.push(diag(
                line_no,
                4,
                "unexpected extra tokens in opening declaration (expected: opening <amount> [COMMODITY])",
              ));
            }

            if decl.opening.is_some() {
              diagnostics.push(diag(line_no, 4, "duplicate opening declaration"));
              continue;
            }

            let amount: f64 = amount_text.parse().unwrap_or(0.0);
            decl.opening = Some(CommodityAmount { commodity, amount });
          }
          _ => {
            diagnostics.push(diag(
              line_no,
              4,
              format!("unknown account declaration entry: {kind}"),
            ));
          }
        }

        continue;
      };

      if current_account.is_some() {
        diagnostics.push(diag(
          line_no,
          0,
          "posting is not allowed inside an account declaration",
        ));
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

      let parsed_amount: f64 = amount.parse().unwrap_or(0.0);
      let remainder_parts: Vec<&str> = parts.collect();
      let remainder = if remainder_parts.is_empty() {
        None
      } else {
        Some(remainder_parts.join(" "))
      };

      txn.postings.push(Posting {
        account: account.to_string(),
        amount: parsed_amount,
        amount_text: amount.to_string(),
        commodity: commodity.to_string(),
        remainder,
      });

      continue;
    }

    // Header line
    if let Some(caps) = header_re.captures(line) {
      flush_transaction(&mut current, &mut diagnostics, &mut transactions);
      if let Some((_, decl)) = current_account.take() {
        account_declarations.push(decl);
      }

      let date = caps
        .name("date")
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "0000-00-00".to_string());
      let time = caps.name("time").map(|m| m.as_str()).unwrap_or("");
      let datetime = format!("{date}{time}");

      let datetime_span = caps.get(0).expect("datetime capture");
      let datetime_end = datetime_span.end();

      let (before_meta, meta) = match line.split_once(';') {
        Some((left, right)) => (left, Some(right.trim().to_string())),
        None => (line, None),
      };

      let header_after_datetime = before_meta.get(datetime_end..).unwrap_or("");
      if header_after_datetime.trim().is_empty() {
        diagnostics.push(diag(line_no, datetime_end, "missing transaction details"));
      }

      if !line.contains(';') {
        diagnostics.push(diag(line_no, 0, "missing meta comment (expected ';')"));
      }

      let (status, payee, narration) = parse_header_fields(header_after_datetime);
      current = Some((
        line_no,
        Transaction {
          date,
          datetime,
          status,
          payee,
          narration,
          meta,
          postings: Vec::new(),
        },
      ));
    } else {
      diagnostics.push(diag(
        line_no,
        0,
        "invalid line: expected transaction header, posting, or directive",
      ));
    }
  }

  flush_transaction(&mut current, &mut diagnostics, &mut transactions);
  if let Some((_, decl)) = current_account.take() {
    account_declarations.push(decl);
  }

  let mut balances_by_account: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
  for txn in &transactions {
    for posting in &txn.postings {
      let entry = balances_by_account
        .entry(posting.account.clone())
        .or_default()
        .entry(posting.commodity.clone())
        .or_insert(0.0);
      *entry += posting.amount;
    }
  }

  for decl in &account_declarations {
    let _ = balances_by_account.entry(decl.account.clone()).or_default();
    if let Some(opening) = &decl.opening {
      let entry = balances_by_account
        .entry(decl.account.clone())
        .or_default()
        .entry(opening.commodity.clone())
        .or_insert(0.0);
      *entry += opening.amount;
    }
  }

  let balances: Vec<AccountBalance> = balances_by_account
    .into_iter()
    .map(|(account, by_commodity)| AccountBalance {
      account,
      totals: by_commodity
        .into_iter()
        .map(|(commodity, amount)| CommodityAmount { commodity, amount })
        .collect(),
    })
    .collect();

  ParseResult {
    ok: diagnostics.is_empty(),
    diagnostics,
    transactions,
    balances,
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
  fn parses_account_declaration_opening_with_default_commodity() {
    let input = r#"account assets:CBA:smartaccess AUD
    opening 100.00
"#;

    let result = parse_transactions(input);
    assert!(result.ok, "expected ok, got diagnostics: {:?}", result.diagnostics);

    let balance = result
      .balances
      .iter()
      .find(|b| b.account == "assets:CBA:smartaccess")
      .expect("expected declared account balance");
    assert_eq!(
      balance.totals,
      vec![CommodityAmount {
        commodity: "AUD".to_string(),
        amount: 100.0
      }]
    );
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
