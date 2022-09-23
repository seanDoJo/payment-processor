# payment-processor

# Assumptions Made
- Disputes and chargebacks made against accounts with insufficient funds (i.e. resulting in negative account balances) are forbidden
- Deposits and withdrawals with amounts <= 0 are forbidden

# Running the utility
```
% cargo run -- example.csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,0.0000,0.0000,0.0000,true

% cargo run -- --verbose example.csv
ERROR - processing Dispute for client 1 with transaction 3

Caused by:
    transaction already disputed
ERROR - processing Dispute for client 2 with transaction 1

Caused by:
    transaction does not exist
ERROR - processing Resolve for client 1 with transaction 1

Caused by:
    transaction is not disputed
ERROR - processing Withdrawal(3.0) for client 2 with transaction 5

Caused by:
    insufficient funds for withdrawal
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,0.0000,0.0000,0.0000,true
```

# Testing
## Unit tests (found in [src/clients.rs](https://github.com/seanDoJo/payment-processor/blob/main/src/clients.rs#L196))
```
% cargo test
running 23 tests
test clients::tests::test_chargeback ... ok
test clients::tests::test_chargeback_unowned_tx ... ok
test clients::tests::test_deposit ... ok
test clients::tests::test_deposit_frozen ... ok
test clients::tests::test_deposit_same_tx ... ok
test clients::tests::test_dispute ... ok
test clients::tests::test_dispute_frozen ... ok
test clients::tests::test_dispute_unowned_tx ... ok
test clients::tests::test_double_chargeback ... ok
test clients::tests::test_double_deposit ... ok
test clients::tests::test_double_dispute ... ok
test clients::tests::test_double_resolve ... ok
test clients::tests::test_hijack_deposit ... ok
test clients::tests::test_resolve ... ok
test clients::tests::test_resolve_frozen ... ok
test clients::tests::test_resolve_unowned_tx ... ok
test clients::tests::test_withdrawal ... ok
test clients::tests::test_withdrawal_frozen ... ok
test clients::tests::test_withdrawal_insufficient ... ok
test clients::tests::test_withdrawal_insufficient_held ... ok
test clients::tests::test_withdrawal_partial_held ... ok
test clients::tests::test_withdrawal_same_tx ... ok
test clients::tests::test_withdrawal_unowned_tx ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Provided test file
`example.csv` located in the root of this repository was used to verify basic event processing
