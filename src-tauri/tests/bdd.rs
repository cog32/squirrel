use cucumber::{given, then, when, World as _};
use squirrel_covid::ledger_parser::{parse_transactions, ParseResult};
use std::path::PathBuf;

#[derive(Debug, Default, cucumber::World)]
struct LedgerWorld {
  file_path: Option<PathBuf>,
  result: Option<ParseResult>,
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

#[tokio::main]
async fn main() {
  let opts = cucumber::cli::Opts::<_, _, _, cucumber::cli::Empty>::parsed();
  let features = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("features")
    .join("parser");

  LedgerWorld::cucumber()
    .with_cli(opts)
    .run_and_exit(features)
    .await;
}
