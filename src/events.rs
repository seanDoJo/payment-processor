use std::fmt;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

/// A raw, unvalidated payment event type for requesting client updates.
#[derive(Clone, Debug, Deserialize)]
pub struct Record {
    /// The type of payment event.
    ///
    /// Expected to be one of
    /// - "deposit"
    /// - "withdrawal"
    /// - "dispute"
    /// - "resolve"
    /// - "chargeback"
    pub r#type: String,
    /// The unique identifier of the client associated with the payment event.
    pub client: u16,
    /// The ID of the transaction associated with the payment event.
    pub tx: u32,
    /// An optional amount of funds associated with the payment event.
    ///
    /// Only valid for [`EventType::Deposit`] and [`EventType::Withdrawal`].
    pub amount: Option<f32>,
}

/// Represents a valid payment event that can be used to attempt to update a client's
/// account state.
#[derive(Clone)]
pub struct Event {
    #[doc(hidden)]
    client: u16,
    #[doc(hidden)]
    tx: u32,
    #[doc(hidden)]
    kind: EventType,
}

/// Represents supported payment event types and any metadata specific to them.
#[derive(Clone, Debug)]
pub enum EventType {
    /// An addition of some funds to a client's account.
    Deposit(f32),
    /// A deduction of some funds from a client's account.
    Withdrawal(f32),
    /// A request to contest the validity of some funds in a client's account.
    Dispute,
    /// A request to validate contested funds of a client's account.
    Resolve,
    /// A request to remove contested funds and freeze a client's account.
    Chargeback,
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} for client {} with transaction {}",
            self.kind, self.client, self.tx
        )
    }
}

impl Event {
    /// Returns the unique identifier of the client associated with the payment event.
    pub fn client_id(&self) -> u16 {
        self.client
    }

    /// Returns the unique identifier of the transaction associated with the payment event.
    pub fn tx(&self) -> u32 {
        self.tx
    }

    /// Returns the type of the payment event and any associated metadata.
    pub fn kind(&self) -> &EventType {
        &self.kind
    }
}

impl TryFrom<Record> for Event {
    type Error = anyhow::Error;

    /// Attempt to create a valid payment event from an un-validated payment record.
    ///
    /// # Example
    /// ```
    /// use payments::events::{Event, Record};
    ///
    /// let valid_record = Record {
    ///     r#type: "deposit",
    ///     client: 1337,
    ///     tx: 1,
    ///     amount: Some(1.0),
    /// };
    ///
    /// // prints "Ok('Deposit(1.0) for client 1337 with transaction 1')"
    /// println!("{:?}", Event::try_from(valid_record));
    ///
    /// let invalid_record = Record {
    ///     r#type: "invalid_event",
    ///     client: 1337,
    ///     tx: 1,
    ///     amount: None,
    /// };
    ///
    /// // prints "Err('invalid transaction type invalid_event')"
    /// println!("{:?}", Event::try_from(invalid_record));
    /// ```
    fn try_from(record: Record) -> Result<Event> {
        Ok(Event {
            client: record.client,
            tx: record.tx,
            kind: match record.r#type.as_str() {
                "deposit" => EventType::Deposit(
                    record
                        .amount
                        .ok_or_else(|| anyhow!("deposit requires an amount"))?,
                ),
                "withdrawal" => EventType::Withdrawal(
                    record
                        .amount
                        .ok_or_else(|| anyhow!("withdrawal requires an  amount"))?,
                ),
                "dispute" => EventType::Dispute,
                "resolve" => EventType::Resolve,
                "chargeback" => EventType::Chargeback,
                v => bail!("invalid transaction type {:?}", v),
            },
        })
    }
}
