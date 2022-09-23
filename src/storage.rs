use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};

/// Represents a client capable of storing and retrieving transactions.
pub trait TxStore: Default {
    /// Returns the requested transaction specified by `tx_id` for the client
    /// specified by `client_id`, if both exist.
    fn get(&self, client_id: u16, tx_id: u32) -> Option<TxState>;
    /// Inserts a new transaction, or updates an existing transaction, specified by
    /// `tx_id`, for the client specified by `client_id`.
    fn upsert(&mut self, client_id: u16, tx_id: u32, tx: TxState) -> Result<()>;
}

/// Defines the amount and current state of a transaction.
#[derive(Clone, Debug)]
pub enum TxState {
    /// A transaction whose funds available for withdrawal.
    Deposit(f32),
    /// A transaction whose funds being held for dispute.
    Dispute(f32),
    /// A transaction representing withdrawn funds.
    Withdrawal,
}

/// An in-memory transaction store backed by a [`HashMap`].
///
/// # Example
/// ```
/// use payments::storage::{MemoryStore, TxState};
///
/// let mut store = MemoryStore::new();
///
/// // insert a transaction with available funds
/// store.upsert(1337, 1, TxState::Deposit(1.0)).unwrap();
/// let tx = store.get(1337, 1).unwrap();
///
/// // prints "Deposit(1.0)"
/// println!("{:?}", tx);
/// ```
#[derive(Default, Debug)]
pub struct MemoryStore {
    #[doc(hidden)]
    transactions: HashMap<u32, (u16, TxState)>,
}

impl MemoryStore {
    pub fn new() -> Arc<Mutex<MemoryStore>> {
        Arc::new(Mutex::new(MemoryStore {
            transactions: HashMap::new(),
        }))
    }
}

impl TxStore for Arc<Mutex<MemoryStore>> {
    fn get(&self, client_id: u16, tx_id: u32) -> Option<TxState> {
        let (cid, tx) = self.lock().unwrap().transactions.get(&tx_id).cloned()?;

        if cid != client_id {
            None
        } else {
            Some(tx)
        }
    }

    fn upsert(&mut self, client_id: u16, tx_id: u32, tx: TxState) -> Result<()> {
        let transactions = &mut self.lock().unwrap().transactions;
        match transactions.get_mut(&tx_id) {
            Some((cid, _)) => {
                if *cid != client_id {
                    bail!("transaction exists for different client");
                }

                transactions.insert(tx_id, (client_id, tx));
                Ok(())
            }
            None => {
                transactions.insert(tx_id, (client_id, tx));
                Ok(())
            }
        }
    }
}
