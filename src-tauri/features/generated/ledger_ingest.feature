Feature: Generated ledger ingestion

  Scenario: Importing a source file appends transactions without editing the source
    Given a clean generated ledger directory
    And a copy of fixture "example.transactions" as a source file
    When I import that source file into the generated ledger for month "202601"
    Then the active ledger should include payee "Kraken"
    And the source file should be unchanged

  Scenario: Manual transaction is appended with a generated txn id
    Given a clean generated ledger directory
    When I add a manual transaction dated "2026-01-20" with payee "Manual" and narration "Test"
    Then the active ledger should include payee "Manual"
    And the active ledger should include meta tag "txn:"

