Feature: Add manual transactions in the UI

  Scenario: Add a manual transaction and see it in the table
    Given the app is running
    When I add a manual transaction with payee "Manual UI" and notes "Coffee"
    Then I should see a transaction row for payee "Manual UI"

