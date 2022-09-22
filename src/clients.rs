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
                    TxState::Deposit(_) => bail!("transaction is not disputed"),
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
                    TxState::Deposit(_) => bail!("transaction is not disputed"),
                }
            }
        };

        Ok(())
    }
}
