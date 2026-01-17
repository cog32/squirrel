Feature: Parse .transactions files

  Scenario: Valid file parses successfully
    Given a transactions file named "valid.transactions"
    When I run the ledger parser on that file
    Then the parse should succeed

  Scenario: Invalid file returns diagnostics
    Given a transactions file named "invalid.transactions"
    When I run the ledger parser on that file
    Then the parse should fail
    And diagnostics should include "missing amount"
