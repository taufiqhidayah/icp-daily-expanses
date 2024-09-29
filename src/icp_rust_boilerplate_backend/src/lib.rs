#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

// Type alias for memory and ID cell types
type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

// Struct representing an Expense entry
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Expense {
    id: u64,
    description: String,
    amount: f64,
    date: u64, // Timestamp for when the expense occurred
    created_at: u64, // Timestamp for when the record was created
    updated_at: Option<u64>, // Optional timestamp for when the record was last updated
}

// Implementing `Storable` for converting `Expense` to and from bytes for storage
impl Storable for Expense {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implementing `BoundedStorable` to define storage constraints for `Expense`
impl BoundedStorable for Expense {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

// Memory manager for stable memory, ID counter, and storage map for expenses
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Expense, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

// Struct for creating/updating an expense with a payload
#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct ExpensePayload {
    description: String,
    amount: f64,
    date: u64, // Timestamp for when the expense occurred
}

// Payload validation function to ensure input is valid
fn validate_expense_payload(payload: &ExpensePayload) -> Result<(), Error> {
    if payload.description.trim().is_empty() {
        return Err(Error::InvalidInput { msg: "Description cannot be empty".to_string() });
    }
    if payload.amount <= 0.0 {
        return Err(Error::InvalidInput { msg: "Amount must be greater than zero".to_string() });
    }
    if payload.date == 0 {
        return Err(Error::InvalidInput { msg: "Date must be a valid timestamp".to_string() });
    }
    Ok(())
}

// Query function to get an expense by ID
#[ic_cdk::query]
fn get_expense(id: u64) -> Result<Expense, Error> {
    match _get_expense(&id) {
        Some(expense) => Ok(expense),
        None => Err(Error::NotFound {
            msg: format!("Expense with id={} not found", id),
        }),
    }
}

// Function to add a new expense
#[ic_cdk::update]
fn add_expense(payload: ExpensePayload) -> Result<Expense, Error> {
    // Validate payload before processing
    validate_expense_payload(&payload)?;

    // Generate a new unique ID
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    // Create a new expense record
    let new_expense = Expense {
        id,
        description: payload.description,
        amount: payload.amount,
        date: payload.date,
        created_at: time(),
        updated_at: None,
    };
    
    // Insert the new expense into storage
    do_insert(&new_expense);
    Ok(new_expense)
}

// Function to update an existing expense
#[ic_cdk::update]
fn update_expense(id: u64, payload: ExpensePayload) -> Result<Expense, Error> {
    // Validate payload before updating
    validate_expense_payload(&payload)?;

    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut expense) => {
            // Update expense details
            expense.description = payload.description;
            expense.amount = payload.amount;
            expense.date = payload.date;
            expense.updated_at = Some(time());

            // Insert the updated expense back into storage
            do_insert(&expense);
            Ok(expense)
        }
        None => Err(Error::NotFound {
            msg: format!("Couldn't update expense with id={}. Expense not found.", id),
        }),
    }
}

// Function to delete an expense by ID
#[ic_cdk::update]
fn delete_expense(id: u64) -> Result<Expense, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(expense) => Ok(expense),
        None => Err(Error::NotFound {
            msg: format!("Couldn't delete expense with id={}. Expense not found.", id),
        }),
    }
}

// Helper function for inserting an expense into storage
fn do_insert(expense: &Expense) {
    STORAGE.with(|service| service.borrow_mut().insert(expense.id, expense.clone()));
}

// Enum to handle custom error responses
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },         // Error when the requested resource is not found
    InvalidInput { msg: String },     // Error for invalid inputs
}

// Helper function to get an expense by ID
fn _get_expense(id: &u64) -> Option<Expense> {
    STORAGE.with(|service| service.borrow().get(id))
}

// Candid export for interface generation
ic_cdk::export_candid!();
