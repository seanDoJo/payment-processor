use crate::events::{Event, EventType};
use crate::storage::{TxState, TxStore};
use anyhow::{anyhow, bail, Result};

/// Represents a client which has some associated transaction history
///
/// # Example
/// ```
/// use payments::clients::Client;
/// use payments::events::{Record, Event};
/// use payments::storage::MemoryStore;
///
/// // create a deposit event for the client
/// let record = Record {
///     r#type: "deposit",
///     client: 1337,
///     tx: 1,
///     amount: Some(1.0),
/// };
/// let event = Event::try_from(record).unwrap();
///
/// // create a new client with id 1337 and an in-memory transaction store
/// let client = Client::new(1337, MemoryStore::new());
/// client.update(&event).unwrap();
///
/// // prints "1.0"
/// println!("{}", client.available());
/// ```
#[derive(Debug, Default)]
pub struct Client<T: TxStore> {
    #[doc(hidden)]
    id: u16,
    #[doc(hidden)]
    available: f32,
    #[doc(hidden)]
    total: f32,
    #[doc(hidden)]
    locked: bool,
    #[doc(hidden)]
    store: T,
}

impl<T: TxStore> Client<T> {
    pub fn new(id: u16, store: T) -> Client<T> {
        Client {
            id,
            store,
            ..Default::default()
        }
    }

    /// Returns the unique identifier of the client.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Returns the funds available for withdrawal.
    pub fn available(&self) -> f32 {
        self.available
    }

    /// Returns the funds held under dispute.
    pub fn held(&self) -> f32 {
        self.total - self.available
    }

    /// Returns the total funds available and held under dispute.
    pub fn total(&self) -> f32 {
        self.total
    }

    /// Returns whether the client's account is frozen.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Updates the client's transaction state based on the provided payment event.
    ///
    /// Client state is updated based on the payment [`EventType`]. If the client's
    /// account is frozen then no update is performed. All events are checked against
    /// the transaction storage layer prior to updating state.
    ///
    ///
    /// [`EventType::Deposit`]
    ///
    /// If the transaction does not already exist then increases the client's
    /// total and available funds by the amount specified
    ///
    /// [`EventType::Withdrawal`]
    ///
    /// If the client's available funds is greater than or equal to the requested
    /// amount then decreases the client's total and available funds by the
    /// amount specified
    ///
    /// [`EventType::Dispute`]
    ///
    /// If the referenced transaction exists and is not already disputed then decrease
    /// the client's available funds by the amount of the specified transaction
    ///
    /// [`EventType::Resolve`]
    ///
    /// If the referenced transaction exists and is disputed then increase the client's
    /// available funds by the amount of the specified transaction
    ///
    /// [`EventType::Chargeback`]
    ///
    /// If the referenced transaction exists and is disputed then decrease the client's
    /// total funds by the amount of the specified transaction and freeze the client's
    /// account
    pub fn update(&mut self, event: &Event) -> Result<()> {
        if self.locked {
            bail!("account is frozen");
        }

        match event.kind() {
            EventType::Deposit(amount) => {
                if self.store.get(self.id, event.tx()).is_some() {
                    bail!("cannot overwrite existing transaction");
                }

                self.store
                    .upsert(self.id, event.tx(), TxState::Deposit(*amount))?;
                self.available += amount;
                self.total += amount;
            }
            EventType::Withdrawal(amount) => {
                if self.available < *amount {
                    bail!("insufficient funds for withdrawal");
                }

                if self.store.get(self.id, event.tx()).is_some() {
                    bail!("cannot overwrite existing transaction");
                }

                self.store
                    .upsert(self.id, event.tx(), TxState::Withdrawal)?;
                self.available -= amount;
                self.total -= amount;
            }
            EventType::Dispute => {
                let tx = self
                    .store
                    .get(self.id, event.tx())
                    .ok_or_else(|| anyhow!("transaction does not exist"))?;
                match tx {
                    TxState::Deposit(amount) => {
                        if amount > self.available {
                            bail!("not enough funds to dispute transaction");
                        }

                        self.store
                            .upsert(self.id, event.tx(), TxState::Dispute(amount))?;
                        self.available -= amount;
                    }
                    TxState::Dispute(_) => bail!("transaction already disputed"),
                    TxState::Withdrawal => bail!("cannot dispute a withdrawal"),
                }
            }
            EventType::Resolve => {
                let tx = self
                    .store
                    .get(self.id, event.tx())
                    .ok_or_else(|| anyhow!("transaction does not exist"))?;
                match tx {
                    TxState::Dispute(amount) => {
                        self.store
                            .upsert(self.id, event.tx(), TxState::Deposit(amount))?;
                        self.available += amount;
                    }
                    TxState::Deposit(_) | TxState::Withdrawal => {
                        bail!("transaction is not disputed")
                    }
                }
            }
            EventType::Chargeback => {
                let tx = self
                    .store
                    .get(self.id, event.tx())
                    .ok_or_else(|| anyhow!("transaction does not exist"))?;
                match tx {
                    TxState::Dispute(amount) => {
                        self.total -= amount;
                        self.locked = true;
                    }
                    TxState::Deposit(_) | TxState::Withdrawal => {
                        bail!("transaction is not disputed")
                    }
                }
            }
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use crate::MemoryStore;
    use crate::Record;

    fn event_with_client(t: &str, client: u16, tx: u32, amount: Option<f32>) -> Event {
        Event::try_from(Record {
            r#type: t.to_string(),
            client,
            tx,
            amount,
        })
        .unwrap()
    }

    fn event(t: &str, tx: u32, amount: Option<f32>) -> Event {
        event_with_client(t, 1337, tx, amount)
    }

    #[test]
    fn test_deposit() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(1.0))).unwrap();
        assert_eq!(client.available(), 1.0);
        assert_eq!(client.held(), 0.0);
        assert_eq!(client.total(), 1.0);
        assert_eq!(client.locked(), false);

        client.update(&event("deposit", 2, Some(10.0))).unwrap();
        assert_eq!(client.available(), 11.0);
        assert_eq!(client.held(), 0.0);
        assert_eq!(client.total(), 11.0);
        assert_eq!(client.locked(), false);
    }

    #[test]
    fn test_deposit_same_tx() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        if let Ok(_) = client.update(&event("deposit", 1, Some(5.0))) {
            panic!("deposit with pre-existing tx id expected to fail")
        }
    }

    #[test]
    fn test_hijack_deposit() {
        let store = MemoryStore::new();
        let mut client = Client::new(1337, Arc::clone(&store));
        client.update(&event("deposit", 1, Some(10.0))).unwrap();

        let mut client = Client::new(1234, Arc::clone(&store));
        if let Ok(_) = client.update(&event_with_client("deposit", 1234, 1, Some(10.0))) {
            panic!("expected deposit of pre-existing tx id for different client to fail")
        }
    }

    #[test]
    fn test_double_deposit() {
        let mut client = Client::new(1337, MemoryStore::new());

        let deposit_event = event("deposit", 1, Some(1.0));
        client.update(&deposit_event).unwrap();
        if let Ok(_) = client.update(&deposit_event) {
            panic!("expected duplicate deposit to fail");
        }
    }

    #[test]
    fn test_deposit_frozen() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(1.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("chargeback", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("deposit", 2, Some(10.0))) {
            panic!("expected deposit to fail for frozen client");
        }
    }

    #[test]
    fn test_withdrawal() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("withdrawal", 2, Some(9.5))).unwrap();
        assert_eq!(client.available(), 0.5);
        assert_eq!(client.held(), 0.0);
        assert_eq!(client.total(), 0.5);
        assert_eq!(client.locked(), false);

        client.update(&event("withdrawal", 3, Some(0.5))).unwrap();
        assert_eq!(client.available(), 0.0);
        assert_eq!(client.held(), 0.0);
        assert_eq!(client.total(), 0.0);
        assert_eq!(client.locked(), false);
    }

    #[test]
    fn test_withdrawal_same_tx() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        if let Ok(_) = client.update(&event("withdrawal", 1, Some(5.0))) {
            panic!("withdrawal with pre-existing tx id expected to fail")
        }
    }

    #[test]
    fn test_withdrawal_unowned_tx() {
        let store = MemoryStore::new();
        let mut client = Client::new(1337, Arc::clone(&store));
        client.update(&event("deposit", 1, Some(10.0))).unwrap();

        let mut client = Client::new(1234, Arc::clone(&store));
        if let Ok(_) = client.update(&event_with_client("withdrawal", 1234, 1, Some(10.0))) {
            panic!("expected withdrawal of tx associated with different client to fail")
        }
    }

    #[test]
    fn test_withdrawal_insufficient() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        if let Ok(_) = client.update(&event("withdrawal", 2, Some(11.0))) {
            panic!("overdraft expected to fail")
        }
    }

    #[test]
    fn test_withdrawal_insufficient_held() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("withdrawal", 2, Some(5.0))) {
            panic!("withdrawal of held funds expected to fail")
        }
    }

    #[test]
    fn test_withdrawal_partial_held() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(5.0))).unwrap();
        client.update(&event("deposit", 2, Some(6.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("withdrawal", 3, Some(5.0))).unwrap();
        assert_eq!(client.available(), 1.0);
        assert_eq!(client.held(), 5.0);
        assert_eq!(client.total(), 6.0);
        assert_eq!(client.locked(), false);
    }

    #[test]
    fn test_withdrawal_frozen() {
        let mut client = Client::new(1337, MemoryStore::new());
        client.update(&event("deposit", 1, Some(5.0))).unwrap();
        client.update(&event("deposit", 2, Some(6.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("chargeback", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("withdrawal", 3, Some(1.0))) {
            panic!("withdrawal from frozen account expected to fail")
        }
    }

    #[test]
    fn test_dispute() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("deposit", 2, Some(5.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        assert_eq!(client.available(), 5.0);
        assert_eq!(client.held(), 10.0);
        assert_eq!(client.total(), 15.0);
        assert_eq!(client.locked(), false);
    }

    #[test]
    fn test_double_dispute() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("dispute", 1, None)) {
            panic!("disputing the same transaction multiple times expected to fail")
        }
    }

    #[test]
    fn test_dispute_unowned_tx() {
        let store = MemoryStore::new();
        let mut client = Client::new(1337, Arc::clone(&store));
        client.update(&event("deposit", 1, Some(10.0))).unwrap();

        let mut client = Client::new(1234, Arc::clone(&store));
        if let Ok(_) = client.update(&event_with_client("dispute", 1234, 1, None)) {
            panic!("dispute tx associated with different client expected to fail")
        }
    }

    #[test]
    fn test_dispute_frozen() {
        let mut client = Client::new(1337, MemoryStore::new());
        client.update(&event("deposit", 1, Some(5.0))).unwrap();
        client.update(&event("deposit", 2, Some(6.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("chargeback", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("dispute", 2, None)) {
            panic!("dispute tx associated with frozen account expected to fail")
        }
    }

    #[test]
    fn test_resolve() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("resolve", 1, None)).unwrap();
        assert_eq!(client.available(), 10.0);
        assert_eq!(client.held(), 0.0);
        assert_eq!(client.total(), 10.0);
        assert_eq!(client.locked(), false);
    }

    #[test]
    fn test_double_resolve() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("resolve", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("resolve", 1, None)) {
            panic!("resolving the same transaction multiple times expected to fail")
        }
    }

    #[test]
    fn test_resolve_unowned_tx() {
        let store = MemoryStore::new();
        let mut client = Client::new(1337, Arc::clone(&store));
        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();

        let mut client = Client::new(1234, Arc::clone(&store));
        if let Ok(_) = client.update(&event_with_client("resolve", 1234, 1, None)) {
            panic!("resolve tx associated with different client expected to fail")
        }
    }

    #[test]
    fn test_resolve_frozen() {
        let mut client = Client::new(1337, MemoryStore::new());
        client.update(&event("deposit", 1, Some(5.0))).unwrap();
        client.update(&event("deposit", 2, Some(6.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("chargeback", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("resolve", 1, None)) {
            panic!("resolve tx associated with frozen account expected to fail")
        }
    }

    #[test]
    fn test_chargeback() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("chargeback", 1, None)).unwrap();
        assert_eq!(client.available(), 0.0);
        assert_eq!(client.held(), 0.0);
        assert_eq!(client.total(), 0.0);
        assert_eq!(client.locked(), true);
    }

    #[test]
    fn test_double_chargeback() {
        let mut client = Client::new(1337, MemoryStore::new());

        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();
        client.update(&event("chargeback", 1, None)).unwrap();
        if let Ok(_) = client.update(&event("chargeback", 1, None)) {
            panic!("chargeback the same transaction multiple times expected to fail")
        }
    }

    #[test]
    fn test_chargeback_unowned_tx() {
        let store = MemoryStore::new();
        let mut client = Client::new(1337, Arc::clone(&store));
        client.update(&event("deposit", 1, Some(10.0))).unwrap();
        client.update(&event("dispute", 1, None)).unwrap();

        let mut client = Client::new(1234, Arc::clone(&store));
        if let Ok(_) = client.update(&event_with_client("chargeback", 1234, 1, None)) {
            panic!("chargeback tx associated with different client expected to fail")
        }
    }
}
