use cucumber::{given, then, when, World as _};
use squirrel_covid::generated_ledger::rotate_ledger_if_needed;
use squirrel_covid::generated_store::{add_manual_transaction, import_source_files, load_active_ledger, ManualPostingInput, ManualTransactionInput};
use squirrel_covid::ledger_parser::{parse_transactions, ParseResult};
use std::path::PathBuf;

#[derive(Debug, Default, cucumber::World)]
struct LedgerWorld {
  file_path: Option<PathBuf>,
  result: Option<ParseResult>,
  generated_dir: Option<PathBuf>,
  source_file_path: Option<PathBuf>,
  source_file_before: Option<String>,
}

fn fixtures_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("features")
    .join("fixtures")
}

#[given(expr = "a transactions file named {string}")]
async fn a_transactions_file_named(world: &mut LedgerWorld, file_name: String) {
  world.file_path = Some(fixtures_dir().join(file_name));
}

#[when("I run the ledger parser on that file")]
async fn i_run_the_ledger_parser_on_that_file(world: &mut LedgerWorld) {
  let file_path = world
    .file_path
    .as_ref()
    .expect("file path should be set by the Given step");
  let contents = std::fs::read_to_string(file_path)
    .unwrap_or_else(|e| panic!("failed to read fixture {file_path:?}: {e}"));
  world.result = Some(parse_transactions(&contents));
}

#[then("the parse should succeed")]
async fn the_parse_should_succeed(world: &mut LedgerWorld) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  assert!(
    result.ok,
    "expected parse ok; diagnostics: {:?}",
    result.diagnostics
  );
}

#[then("the parse should fail")]
async fn the_parse_should_fail(world: &mut LedgerWorld) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  assert!(!result.ok, "expected parse failure");
}

#[then(expr = "diagnostics should include {string}")]
async fn diagnostics_should_include(world: &mut LedgerWorld, needle: String) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  assert!(
    result
      .diagnostics
      .iter()
      .any(|d| d.message.contains(&needle)),
    "expected diagnostics to include {needle:?}; got: {:?}",
    result.diagnostics
  );
}

#[then(expr = "the first transaction payee should be {string}")]
async fn first_transaction_payee_should_be(world: &mut LedgerWorld, expected: String) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  let txn = result
    .transactions
    .first()
    .expect("expected at least one parsed transaction");
  assert_eq!(
    txn.payee.as_deref(),
    Some(expected.as_str()),
    "unexpected payee: {:?}",
    txn.payee
  );
}

#[then(expr = "the first transaction narration should be {string}")]
async fn first_transaction_narration_should_be(world: &mut LedgerWorld, expected: String) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  let txn = result
    .transactions
    .first()
    .expect("expected at least one parsed transaction");
  assert_eq!(
    txn.narration.as_deref(),
    Some(expected.as_str()),
    "unexpected narration: {:?}",
    txn.narration
  );
}

#[then(expr = "the first transaction meta should include {string}")]
async fn first_transaction_meta_should_include(world: &mut LedgerWorld, expected: String) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  let txn = result
    .transactions
    .first()
    .expect("expected at least one parsed transaction");
  let meta = txn.meta.as_deref().unwrap_or("");
  assert!(
    meta.contains(&expected),
    "expected meta to include {expected:?}, got: {meta:?}"
  );
}

#[then(expr = "the balance for account {string} should be {string} {string}")]
async fn the_balance_for_account_should_be(
  world: &mut LedgerWorld,
  account: String,
  amount_text: String,
  commodity: String,
) {
  let result = world
    .result
    .as_ref()
    .expect("parse result should be set by the When step");
  let expected_amount: f64 = amount_text
    .parse()
    .unwrap_or_else(|e| panic!("invalid expected amount {amount_text:?}: {e}"));
  let balance = result
    .balances
    .iter()
    .find(|b| b.account == account)
    .unwrap_or_else(|| panic!("missing balance for account {account:?}"));
  let actual = balance
    .totals
    .iter()
    .find(|t| t.commodity == commodity)
    .unwrap_or_else(|| panic!("missing commodity {commodity:?} for account {account:?}"));

  assert!(
    (actual.amount - expected_amount).abs() < 1e-9,
    "expected {expected_amount} {commodity} for {account}, got {actual:?}",
  );
}

fn new_temp_dir(prefix: &str) -> PathBuf {
  let nanos = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_nanos();
  let pid = std::process::id();
  std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"))
}

#[given("a clean generated ledger directory")]
async fn a_clean_generated_ledger_directory(world: &mut LedgerWorld) {
  let base_dir = new_temp_dir("squirrel-covid-generated-ledger");
  std::fs::create_dir_all(&base_dir).expect("create temp dir");
  world.generated_dir = Some(base_dir);
  world.source_file_path = None;
  world.source_file_before = None;
}

#[given(expr = "a copy of fixture {string} as a source file")]
async fn a_copy_of_fixture_as_a_source_file(world: &mut LedgerWorld, fixture: String) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let src = fixtures_dir().join(fixture);
  let dest = dir.join("source.transactions");
  let contents = std::fs::read_to_string(&src).expect("read fixture");
  std::fs::write(&dest, &contents).expect("write copied source");
  world.source_file_path = Some(dest);
  world.source_file_before = Some(contents);
}

#[when(expr = "I import that source file into the generated ledger for month {string}")]
async fn i_import_that_source_file_into_the_generated_ledger_for_month(
  world: &mut LedgerWorld,
  now_yyyymm: String,
) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let source = world
    .source_file_path
    .as_ref()
    .expect("source file should be set by the Given step");
  import_source_files(dir, &now_yyyymm, &[source.display().to_string()]).expect("import sources");
}

#[then(expr = "the active ledger should include payee {string}")]
async fn the_active_ledger_should_include_payee(world: &mut LedgerWorld, payee: String) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let result = load_active_ledger(dir).expect("load active ledger");
  assert!(
    result
      .transactions
      .iter()
      .any(|t| t.payee.as_deref() == Some(payee.as_str())),
    "expected active ledger to include payee {payee:?}; got: {:?}",
    result
      .transactions
      .iter()
      .map(|t| t.payee.clone())
      .collect::<Vec<_>>()
  );
}

#[then("the source file should be unchanged")]
async fn the_source_file_should_be_unchanged(world: &mut LedgerWorld) {
  let source = world
    .source_file_path
    .as_ref()
    .expect("source file should be set by the Given step");
  let before = world
    .source_file_before
    .as_ref()
    .expect("source file contents should be set by the Given step");
  let after = std::fs::read_to_string(source).expect("read source after");
  assert_eq!(before, &after, "expected source file to be unchanged");
}

#[when(expr = "I add a manual transaction dated {string} with payee {string} and narration {string}")]
async fn i_add_a_manual_transaction_dated_with_payee_and_narration(
  world: &mut LedgerWorld,
  date: String,
  payee: String,
  narration: String,
) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let now_yyyymm = date.get(0..7).map(|s| s.replace('-', "")).unwrap_or_else(|| "000000".to_string());
  let input = ManualTransactionInput {
    datetime: date,
    status: Some('*'),
    payee,
    narration,
    postings: vec![
      ManualPostingInput {
        account: "assets:cash:usd".to_string(),
        amount: "1.00".to_string(),
        commodity: "USD".to_string(),
        remainder: None,
      },
      ManualPostingInput {
        account: "expenses:manual:test".to_string(),
        amount: "-1.00".to_string(),
        commodity: "USD".to_string(),
        remainder: None,
      },
    ],
  };
  add_manual_transaction(dir, &now_yyyymm, &input).expect("add manual transaction");
}

#[then(expr = "the active ledger should include meta tag {string}")]
async fn the_active_ledger_should_include_meta_tag(world: &mut LedgerWorld, needle: String) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let contents = std::fs::read_to_string(dir.join("ledger.transactions")).unwrap_or_default();
  assert!(
    contents.contains(&needle),
    "expected active ledger to include {needle:?}; got: {contents:?}"
  );
}

#[given(expr = "a generated ledger file for month {string}")]
async fn a_generated_ledger_file_for_month(world: &mut LedgerWorld, yyyymm: String) {
  let year: i32 = yyyymm[0..4].parse().expect("year");
  let month: i32 = yyyymm[4..6].parse().expect("month");
  let date = format!("{year:04}-{month:02}-16");

  let base_dir = new_temp_dir("squirrel-covid-generated-ledger");
  std::fs::create_dir_all(&base_dir).expect("create temp dir");
  let ledger = base_dir.join("ledger.transactions");

  let contents = format!(
    r#"{date} * "Kraken" "Sell BTC" ; txn:01J2NB..., src:kraken:trade:def456
    assets:cash:usd               160.00 USD

"#
  );
  std::fs::write(&ledger, contents).expect("write ledger.transactions");
  world.generated_dir = Some(base_dir);
}

#[when(expr = "I rotate the generated ledger with current month {string}")]
async fn i_rotate_the_generated_ledger_with_current_month(world: &mut LedgerWorld, now_yyyymm: String) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  rotate_ledger_if_needed(dir, &now_yyyymm).expect("rotate ledger");
}

#[then(expr = "the archive ledger file {string} should exist")]
async fn the_archive_ledger_file_should_exist(world: &mut LedgerWorld, file_name: String) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let path = dir.join("archive").join(file_name);
  assert!(path.exists(), "expected archive file to exist: {path:?}");
}

#[then(expr = "the archive ledger file {string} should not exist")]
async fn the_archive_ledger_file_should_not_exist(world: &mut LedgerWorld, file_name: String) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let path = dir.join("archive").join(file_name);
  assert!(
    !path.exists(),
    "expected archive file to not exist: {path:?}"
  );
}

#[then("the active ledger file should be empty")]
async fn the_active_ledger_file_should_be_empty(world: &mut LedgerWorld) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let contents =
    std::fs::read_to_string(dir.join("ledger.transactions")).expect("read ledger.transactions");
  assert!(
    contents.trim().is_empty(),
    "expected ledger.transactions to be empty, got: {contents:?}"
  );
}

#[then("the active ledger file should not be empty")]
async fn the_active_ledger_file_should_not_be_empty(world: &mut LedgerWorld) {
  let dir = world
    .generated_dir
    .as_ref()
    .expect("generated dir should be set by the Given step");
  let contents =
    std::fs::read_to_string(dir.join("ledger.transactions")).expect("read ledger.transactions");
  assert!(
    !contents.trim().is_empty(),
    "expected ledger.transactions to not be empty"
  );
}

#[tokio::main]
async fn main() {
  let opts = cucumber::cli::Opts::<_, _, _, cucumber::cli::Empty>::parsed();
  let features = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("features");

  LedgerWorld::cucumber()
    .with_cli(opts)
    .run_and_exit(features)
    .await;
}
